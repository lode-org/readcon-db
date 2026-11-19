use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use heed::types::Bytes;
use heed::{Database, EnvOpenOptions};
use readcon_db::{
    ClaimError, ClaimStatus, ConCorpus, Error, MAX_CATALOG_COMPONENT_BYTES,
    MAX_CATALOG_RECORD_BYTES,
};
use std::time::Instant;

/// Start B this far before `return + L` so the call can return while still
/// inside the lease. This is scheduling room, not the lease margin.
const EARLY_BY: Duration = Duration::from_millis(5);
/// `ceil` adds less than 1ms, the return margin is 1ms, and strict
/// `expiry < now` waits one more millisecond. Polling for takeover starts
/// at `return + L + 3ms`.
const TAKEOVER_AFTER_LEASE: Duration = Duration::from_millis(3);
/// How long one attempt keeps polling after `L + 3ms`. A scheduler bound
/// for this test, not a lease margin.
const ACQUIRE_POLL_LIMIT: Duration = Duration::from_secs(2);
/// Guarded `Busy` assertions this test must execute before it can pass.
const MIN_BUSY_ASSERTIONS: usize = 1;
/// Stop opening new attempts once this elapses. A scheduler bound, not a
/// lease margin. A run that never executes a guarded `Busy` assertion, or
/// never observes takeover, fails.
const REALTIME_RETRY_LIMIT: Duration = Duration::from_secs(30);

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

fn sleep_until(start: Instant, target: Duration) {
    loop {
        let elapsed = start.elapsed();
        if elapsed >= target {
            return;
        }
        let left = target - elapsed;
        if left > Duration::from_millis(20) {
            thread::sleep(left - Duration::from_millis(10));
        } else {
            thread::sleep(Duration::from_micros(50));
        }
    }
}

#[test]
fn lease_duration_is_never_shorter_than_lease_secs() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = ConCorpus::open(dir.path()).unwrap();
    let lease = Duration::from_secs(1);
    let poll_from = lease + TAKEOVER_AFTER_LEASE;
    let started = Instant::now();
    let mut busy_assertions = 0usize;
    let mut takeover_seen = false;
    let mut attempt = 0usize;
    while started.elapsed() < REALTIME_RETRY_LIMIT
        && (busy_assertions < MIN_BUSY_ASSERTIONS || !takeover_seen)
    {
        attempt += 1;
        let task = format!("task-duration-{attempt}");
        let task = task.as_bytes();
        let before_a = Instant::now();
        assert_eq!(
            corpus.try_claim(b"jobs", task, b"owner-a", 1).unwrap(),
            ClaimStatus::Acquired
        );
        let returned = Instant::now();
        // The stored expiry is sampled before commit. The return+L bound
        // applies only when that commit and the return finish within 1ms.
        let claim_within_margin = before_a.elapsed() < Duration::from_millis(1);
        sleep_until(returned, lease.saturating_sub(EARLY_BY));
        if claim_within_margin && returned.elapsed() < lease {
            let status = corpus.try_claim(b"jobs", task, b"owner-b", 1).unwrap();
            let after_b_elapsed = returned.elapsed();
            // B's own call can cross the lease. That iteration is not an
            // eligible Busy check; a later attempt retries the window.
            if after_b_elapsed < lease {
                busy_assertions += 1;
                assert_eq!(
                    status,
                    ClaimStatus::Busy,
                    "attempt {attempt}: elapsed {after_b_elapsed:?} is still inside the lease"
                );
            }
        }
        let attempt_deadline = poll_from + ACQUIRE_POLL_LIMIT;
        loop {
            let elapsed = returned.elapsed();
            if elapsed >= poll_from {
                let status = corpus.try_claim(b"jobs", task, b"owner-c", 1).unwrap();
                if status == ClaimStatus::Acquired {
                    takeover_seen = true;
                    break;
                }
                assert_eq!(
                    status,
                    ClaimStatus::Busy,
                    "attempt {attempt}: unexpected status at {elapsed:?}"
                );
            }
            if elapsed >= attempt_deadline {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
    }
    println!(
        "lease_duration_is_never_shorter_than_lease_secs asserting_iterations={busy_assertions} takeover_seen={takeover_seen} attempts={attempt}"
    );
    assert!(
        busy_assertions >= MIN_BUSY_ASSERTIONS,
        "guarded Busy assertions ran {busy_assertions} times in {attempt} attempts"
    );
    assert!(
        takeover_seen,
        "takeover was not observed in {attempt} attempts"
    );
}
