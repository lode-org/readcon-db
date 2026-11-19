use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

use heed::types::Bytes;
use heed::{Database, EnvOpenOptions};
use readcon_db::{
    ClaimError, ClaimStatus, Clock, ConCorpus, Error, MAX_CATALOG_COMPONENT_BYTES,
    MAX_CATALOG_RECORD_BYTES,
};

const LEASE_SECS: u64 = 60;

fn prepatch_db_names() -> &'static [&'static str] {
    &[
        "frames",
        "traj_meta",
        "idx_natoms",
        "idx_symbol",
        "idx_energy",
        "idx_flags",
        "idx_elem_count",
        "idx_formula",
        "idx_fmax",
        "idx_mass",
        "idx_volume",
        "idx_pbc",
        "idx_meta",
        "frame_by_hash",
        "hash_by_frame",
        "frames_soa",
        "idx_topo",
        "topo_by_frame",
        "idx_canon",
        "canon_by_frame",
    ]
}

#[test]
fn concurrent_catalog_insert_deduplicates_and_is_immediately_visible() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = Arc::new(ConCorpus::open(dir.path()).unwrap());
    let start = Arc::new(Barrier::new(12));
    let committed = Arc::new(Barrier::new(12));
    let inserted = Arc::new(AtomicUsize::new(0));

    let threads: Vec<_> = (0..12)
        .map(|worker| {
            let corpus = Arc::clone(&corpus);
            let start = Arc::clone(&start);
            let committed = Arc::clone(&committed);
            let inserted = Arc::clone(&inserted);
            thread::spawn(move || {
                start.wait();
                if corpus
                    .insert_catalog_record(
                        b"search",
                        b"same-subject",
                        b"same-fingerprint",
                        format!("worker-{worker}").as_bytes(),
                    )
                    .unwrap()
                {
                    inserted.fetch_add(1, Ordering::SeqCst);
                }
                committed.wait();

                let records = corpus
                    .list_catalog_records(b"search", b"same-subject")
                    .unwrap();
                assert_eq!(records.len(), 1);
                assert_eq!(records[0].fingerprint, b"same-fingerprint");
            })
        })
        .collect();

    for handle in threads {
        handle.join().unwrap();
    }
    assert_eq!(inserted.load(Ordering::SeqCst), 1);
}

#[test]
fn exactly_one_thread_claims_and_completion_is_atomically_visible() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = Arc::new(ConCorpus::open(dir.path()).unwrap());
    let start = Arc::new(Barrier::new(16));

    let threads: Vec<_> = (0..16)
        .map(|worker| {
            let corpus = Arc::clone(&corpus);
            let start = Arc::clone(&start);
            thread::spawn(move || {
                let owner = format!("owner-{worker}").into_bytes();
                start.wait();
                let status = corpus
                    .try_claim(b"jobs", b"task-1", &owner, LEASE_SECS)
                    .unwrap();
                (owner, status)
            })
        })
        .collect();

    let results: Vec<_> = threads
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|(_, status)| *status == ClaimStatus::Acquired)
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|(_, status)| *status == ClaimStatus::Busy)
            .count(),
        15
    );
    let winner = results
        .iter()
        .find(|(_, status)| *status == ClaimStatus::Acquired)
        .unwrap()
        .0
        .clone();
    assert_eq!(
        corpus
            .try_claim(b"jobs", b"task-1", &winner, LEASE_SECS)
            .unwrap(),
        ClaimStatus::AlreadyOwned
    );

    assert!(corpus
        .complete_claim(
            b"jobs",
            b"task-1",
            b"not-the-owner",
            b"result",
            b"fingerprint",
            b"wrong",
        )
        .is_err());
    assert!(corpus
        .list_catalog_records(b"jobs", b"result")
        .unwrap()
        .is_empty());

    let visibility = Arc::new(Barrier::new(2));
    let reader = {
        let corpus = Arc::clone(&corpus);
        let visibility = Arc::clone(&visibility);
        thread::spawn(move || {
            visibility.wait();
            let records = corpus.list_catalog_records(b"jobs", b"result").unwrap();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].value, b"answer");
            assert_eq!(
                corpus
                    .try_claim(b"jobs", b"task-1", b"late-owner", LEASE_SECS)
                    .unwrap(),
                ClaimStatus::Complete
            );
        })
    };
    assert!(corpus
        .complete_claim(
            b"jobs",
            b"task-1",
            &winner,
            b"result",
            b"fingerprint",
            b"answer",
        )
        .unwrap());
    visibility.wait();
    reader.join().unwrap();
}

#[test]
fn release_requires_the_owner_and_inputs_are_not_defaulted() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = ConCorpus::open(dir.path()).unwrap();

    assert_eq!(
        corpus
            .try_claim(b"jobs", b"task-2", b"owner-a", LEASE_SECS)
            .unwrap(),
        ClaimStatus::Acquired
    );
    assert!(corpus
        .release_claim(b"jobs", b"task-2", b"owner-b")
        .is_err());
    assert_eq!(
        corpus
            .try_claim(b"jobs", b"task-2", b"owner-b", LEASE_SECS)
            .unwrap(),
        ClaimStatus::Busy
    );
    corpus
        .release_claim(b"jobs", b"task-2", b"owner-a")
        .unwrap();
    assert_eq!(
        corpus
            .try_claim(b"jobs", b"task-2", b"owner-b", LEASE_SECS)
            .unwrap(),
        ClaimStatus::Acquired
    );

    assert!(corpus
        .insert_catalog_record(b"", b"subject", b"fingerprint", b"value")
        .is_err());
    let oversized_namespace = [b'n'; MAX_CATALOG_COMPONENT_BYTES + 1];
    assert!(corpus
        .insert_catalog_record(&oversized_namespace, b"subject", b"fingerprint", b"value",)
        .is_err());
    assert!(corpus
        .insert_catalog_record(b"namespace", b"subject", b"fingerprint", b"")
        .is_err());
    assert!(corpus
        .insert_catalog_record(
            b"namespace",
            b"subject",
            b"fingerprint",
            &vec![0; MAX_CATALOG_RECORD_BYTES + 1],
        )
        .is_err());
}

#[test]
fn readonly_catalog_reads_and_all_mutations_fail_explicitly() {
    let dir = tempfile::tempdir().unwrap();
    {
        let corpus = ConCorpus::open(dir.path()).unwrap();
        corpus
            .insert_catalog_record(b"namespace", b"subject", b"fingerprint", b"value")
            .unwrap();
    }
    let corpus = ConCorpus::open_readonly(dir.path()).unwrap();
    assert_eq!(
        corpus
            .list_catalog_records(b"namespace", b"subject")
            .unwrap()
            .len(),
        1
    );
    assert!(corpus
        .insert_catalog_record(b"namespace", b"subject", b"new-fingerprint", b"value")
        .unwrap_err()
        .to_string()
        .contains("readonly"));
    assert!(corpus
        .try_claim(b"namespace", b"task", b"owner", LEASE_SECS)
        .unwrap_err()
        .to_string()
        .contains("readonly"));
}

#[test]
fn readonly_older_corpus_treats_absent_catalog_databases_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut options = EnvOpenOptions::new();
        options.map_size(16 * 1024 * 1024).max_dbs(48);
        let env = unsafe { options.open(dir.path()).unwrap() };
        let mut txn = env.write_txn().unwrap();
        for name in prepatch_db_names() {
            let _: Database<Bytes, Bytes> = env.create_database(&mut txn, Some(name)).unwrap();
        }
        txn.commit().unwrap();
    }

    let corpus = ConCorpus::open_readonly(dir.path()).unwrap();
    assert!(corpus
        .list_catalog_records(b"namespace", b"subject")
        .unwrap()
        .is_empty());
    assert!(corpus
        .try_claim(b"namespace", b"task", b"owner", LEASE_SECS)
        .unwrap_err()
        .to_string()
        .contains("readonly"));
}

#[test]
fn list_catalog_records_is_scoped_by_prefix_namespace_and_subject() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = ConCorpus::open(dir.path()).unwrap();
    corpus
        .insert_catalog_record(b"ns-a", b"sub-1", b"fp-a", b"value-a")
        .unwrap();
    corpus
        .insert_catalog_record(b"ns-a", b"sub-11", b"fp-b", b"value-b")
        .unwrap();
    corpus
        .insert_catalog_record(b"ns-ab", b"sub-1", b"fp-c", b"value-c")
        .unwrap();
    corpus
        .insert_catalog_record(b"ns-a", b"sub-1", b"fp-d", b"value-d")
        .unwrap();

    let records = corpus.list_catalog_records(b"ns-a", b"sub-1").unwrap();
    let mut seen = records
        .iter()
        .map(|record| record.fingerprint.clone())
        .collect::<Vec<_>>();
    seen.sort();
    assert_eq!(seen, vec![b"fp-a".to_vec(), b"fp-d".to_vec()]);
}

#[test]
fn lease_expiry_allows_reclaim_and_blocks_lapsed_complete() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = ConCorpus::open(dir.path()).unwrap();
    assert_eq!(
        corpus.try_claim(b"jobs", b"task-3", b"owner-a", 1).unwrap(),
        ClaimStatus::Acquired
    );
    thread::sleep(Duration::from_secs(2));
    assert_eq!(
        corpus
            .try_claim(b"jobs", b"task-3", b"owner-b", LEASE_SECS)
            .unwrap(),
        ClaimStatus::Acquired
    );
    let err = corpus
        .complete_claim(
            b"jobs",
            b"task-3",
            b"owner-a",
            b"result",
            b"fingerprint",
            b"value",
        )
        .unwrap_err();
    assert!(matches!(err, Error::Claim(ClaimError::NotOwner)));
}

#[test]
fn zero_lease_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = ConCorpus::open(dir.path()).unwrap();
    let err = corpus
        .try_claim(b"jobs", b"task-zero-lease", b"owner-a", 0)
        .unwrap_err();
    assert!(err
        .to_string()
        .contains("lease_secs must be greater than zero"));
}

#[test]
fn completing_expired_claim_before_takeover_is_expired() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = ConCorpus::open(dir.path()).unwrap();
    assert_eq!(
        corpus
            .try_claim(b"jobs", b"task-expired-complete", b"owner-a", 1)
            .unwrap(),
        ClaimStatus::Acquired
    );
    thread::sleep(Duration::from_secs(2));
    let err = corpus
        .complete_claim(
            b"jobs",
            b"task-expired-complete",
            b"owner-a",
            b"result",
            b"fingerprint",
            b"value",
        )
        .unwrap_err();
    assert!(matches!(err, Error::Claim(ClaimError::Expired)));
}

#[test]
fn owner_can_reclaim_its_own_expired_task() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = ConCorpus::open(dir.path()).unwrap();
    assert_eq!(
        corpus
            .try_claim(b"jobs", b"task-self-reclaim", b"owner-a", 1)
            .unwrap(),
        ClaimStatus::Acquired
    );
    thread::sleep(Duration::from_secs(2));
    assert_eq!(
        corpus
            .try_claim(b"jobs", b"task-self-reclaim", b"owner-a", 1)
            .unwrap(),
        ClaimStatus::Acquired
    );
}

#[test]
fn renew_claim_updates_lease_and_is_owner_checked() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = ConCorpus::open(dir.path()).unwrap();
    assert_eq!(
        corpus
            .try_claim(b"jobs", b"task-4", b"owner-a", LEASE_SECS)
            .unwrap(),
        ClaimStatus::Acquired
    );
    corpus
        .renew_claim(b"jobs", b"task-4", b"owner-a", LEASE_SECS)
        .unwrap();
    let err = corpus
        .renew_claim(b"jobs", b"task-4", b"owner-b", LEASE_SECS)
        .unwrap_err();
    assert!(matches!(err, Error::Claim(ClaimError::NotOwner)));
}

#[test]
fn claim_errors_are_typed_for_unclaimed_complete_and_expired() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = ConCorpus::open(dir.path()).unwrap();

    let err = corpus
        .release_claim(b"jobs", b"missing", b"owner")
        .unwrap_err();
    assert!(matches!(err, Error::Claim(ClaimError::Unclaimed)));

    assert_eq!(
        corpus
            .try_claim(b"jobs", b"task-5", b"owner-a", LEASE_SECS)
            .unwrap(),
        ClaimStatus::Acquired
    );
    assert!(corpus
        .complete_claim(
            b"jobs",
            b"task-5",
            b"owner-a",
            b"result",
            b"fingerprint",
            b"value",
        )
        .unwrap());
    let err = corpus
        .release_claim(b"jobs", b"task-5", b"owner-a")
        .unwrap_err();
    assert!(matches!(err, Error::Claim(ClaimError::AlreadyComplete)));

    assert_eq!(
        corpus.try_claim(b"jobs", b"task-6", b"owner-a", 1).unwrap(),
        ClaimStatus::Acquired
    );
    thread::sleep(Duration::from_secs(2));
    let err = corpus
        .renew_claim(b"jobs", b"task-6", b"owner-a", LEASE_SECS)
        .unwrap_err();
    assert!(matches!(err, Error::Claim(ClaimError::Expired)));
}

#[test]
fn open_readwrite_backfills_new_catalog_dbs_and_readonly_listing_is_empty_on_prepatch() {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut options = EnvOpenOptions::new();
        options.map_size(16 * 1024 * 1024).max_dbs(48);
        let env = unsafe { options.open(dir.path()).unwrap() };
        let mut txn = env.write_txn().unwrap();
        for name in prepatch_db_names() {
            let _: Database<Bytes, Bytes> = env.create_database(&mut txn, Some(name)).unwrap();
        }
        txn.commit().unwrap();
    }

    let ro = ConCorpus::open_readonly(dir.path()).unwrap();
    assert!(ro
        .list_catalog_records(b"legacy", b"subject")
        .unwrap()
        .is_empty());
    ro.close();

    {
        let rw = ConCorpus::open(dir.path()).unwrap();
        rw.close();
    }

    let mut options = EnvOpenOptions::new();
    options.map_size(16 * 1024 * 1024).max_dbs(48);
    let env = unsafe { options.open(dir.path()).unwrap() };
    let rtxn = env.read_txn().unwrap();
    let catalog_records: Option<Database<Bytes, Bytes>> =
        env.open_database(&rtxn, Some("catalog_records")).unwrap();
    let work_claims: Option<Database<Bytes, Bytes>> =
        env.open_database(&rtxn, Some("work_claims")).unwrap();
    assert!(catalog_records.is_some());
    assert!(work_claims.is_some());
}

/// A clock this test moves by hand, so no assertion depends on how fast the
/// host runs. `stall` is the virtual time that passes on every sample, which
/// models a host slow enough that the clock moves while one claim call runs.
/// A claim decides from the first sample it takes, so the instant
/// [`TestClock::sample_at`] sets is the one that call acts on whatever the
/// stall is.
#[derive(Debug)]
struct TestClock {
    nanos: Mutex<u128>,
    stall: u128,
}

impl TestClock {
    fn with_stall(stall: u128) -> Arc<Self> {
        Arc::new(Self {
            nanos: Mutex::new(0),
            stall,
        })
    }

    fn sample_at(&self, nanos: u128) {
        *self.nanos.lock().unwrap() = nanos;
    }
}

impl Clock for TestClock {
    fn unix_time_nanos(&self) -> readcon_db::Result<u128> {
        let mut nanos = self.nanos.lock().unwrap();
        let sampled = *nanos;
        *nanos += self.stall;
        Ok(sampled)
    }
}

/// Unix nanoseconds on a millisecond boundary. Cases offset into that
/// millisecond so the `ceil` in the stored deadline is exercised.
const CLOCK_BASE_NS: u128 = 1_700_000_000_000 * 1_000_000;
/// `ceil` adds under one millisecond, the stored deadline carries a 1ms
/// return margin, and strict `expiry < now` costs one more millisecond.
/// Takeover therefore cannot be later than `claim + lease + 3ms`.
const TAKEOVER_BOUND_AFTER_LEASE_NS: u128 = 3_000_000;
const NANOS_PER_SEC: u128 = 1_000_000_000;

/// Each case is `(lease_secs, offset into the starting millisecond, stall)`.
const LEASE_WINDOW_CASES: &[(u64, u128, u128)] = &[
    (1, 0, 0),
    (1, 1, 0),
    (1, 999_999, 0),
    (60, 500_000, 0),
    // Slow hosts: a quarter and three quarters of a second of clock pass
    // inside every claim call, as they would on a loaded runner.
    (1, 0, 250_000_000),
    (1, 999_999, 750_000_000),
];

/// Claim a never-used task at `claim_ns`, then ask for it as another owner at
/// `probe_ns`. Each probe owns its task, so a takeover here cannot change what
/// a later probe sees.
fn probe_lease(
    corpus: &ConCorpus,
    clock: &TestClock,
    probe: usize,
    lease_secs: u64,
    claim_ns: u128,
    probe_ns: u128,
) -> ClaimStatus {
    let task = format!("task-duration-{probe}");
    let task = task.as_bytes();
    clock.sample_at(claim_ns);
    assert_eq!(
        corpus
            .try_claim(b"jobs", task, b"owner-a", lease_secs)
            .unwrap(),
        ClaimStatus::Acquired
    );
    clock.sample_at(probe_ns);
    corpus
        .try_claim(b"jobs", task, b"owner-b", lease_secs)
        .unwrap()
}

#[test]
fn lease_duration_is_never_shorter_than_lease_secs() {
    for &(lease_secs, offset_ns, stall_ns) in LEASE_WINDOW_CASES {
        let dir = tempfile::tempdir().unwrap();
        let clock = TestClock::with_stall(stall_ns);
        let corpus = ConCorpus::open(dir.path())
            .unwrap()
            .with_clock(clock.clone());
        let case = format!("lease {lease_secs}s, offset {offset_ns}ns, stall {stall_ns}ns");
        let claim_ns = CLOCK_BASE_NS + offset_ns;
        let lease_ns = u128::from(lease_secs) * NANOS_PER_SEC;
        let mut probe = 0usize;

        // Inside the lease, including its very last nanosecond, the task is
        // held: the lease is never shorter than lease_secs.
        for inside_ns in [claim_ns, claim_ns + lease_ns / 2, claim_ns + lease_ns] {
            probe += 1;
            assert_eq!(
                probe_lease(&corpus, &clock, probe, lease_secs, claim_ns, inside_ns),
                ClaimStatus::Busy,
                "{case}: another owner took the task {}ns after the claim",
                inside_ns - claim_ns
            );
        }

        // Bisect the first instant another owner takes over. `busy_ns` stays
        // held and `free_ns` stays taken over, so they close on the boundary.
        let mut busy_ns = claim_ns + lease_ns;
        let mut free_ns = busy_ns + TAKEOVER_BOUND_AFTER_LEASE_NS;
        probe += 1;
        assert_eq!(
            probe_lease(&corpus, &clock, probe, lease_secs, claim_ns, free_ns),
            ClaimStatus::Acquired,
            "{case}: the expired task was not taken over at lease + 3ms"
        );
        while free_ns - busy_ns > 1 {
            let mid_ns = busy_ns + (free_ns - busy_ns) / 2;
            probe += 1;
            match probe_lease(&corpus, &clock, probe, lease_secs, claim_ns, mid_ns) {
                ClaimStatus::Busy => busy_ns = mid_ns,
                ClaimStatus::Acquired => free_ns = mid_ns,
                status => panic!(
                    "{case}: unexpected {status:?} {}ns after the claim",
                    mid_ns - claim_ns
                ),
            }
        }
        assert!(
            free_ns > claim_ns + lease_ns,
            "{case}: takeover at {}ns is inside the lease",
            free_ns - claim_ns
        );
        println!(
            "lease_duration_is_never_shorter_than_lease_secs case=({lease_secs},{offset_ns},{stall_ns}) takeover_after_claim_ns={} probes={probe}",
            free_ns - claim_ns
        );
    }
}
