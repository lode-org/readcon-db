use std::sync::{Arc, Mutex};

use readcon_db::{join_drained_roots, ClaimStatus, Clock, ConCorpus, ShardedConCorpus};

const BASE_NS: u128 = 1_700_000_000_000 * 1_000_000;
const NANOS_PER_MILLI: u128 = 1_000_000;

#[derive(Debug)]
struct TestClock {
    nanos: Mutex<u128>,
}

impl TestClock {
    fn at(nanos: u128) -> Arc<Self> {
        Arc::new(Self {
            nanos: Mutex::new(nanos),
        })
    }

    fn set(&self, nanos: u128) {
        *self.nanos.lock().unwrap() = nanos;
    }
}

impl Clock for TestClock {
    fn unix_time_nanos(&self) -> readcon_db::Result<u128> {
        Ok(*self.nanos.lock().unwrap())
    }
}

#[test]
fn finished_claim_survives_shard_join() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sharded");
    ShardedConCorpus::open(&root, 2).unwrap();
    let shard = ShardedConCorpus::open_shard(&root, 0)
        .unwrap()
        .with_clock(TestClock::at(BASE_NS));
    assert_eq!(
        shard
            .try_claim(b"jobs", b"finished", b"owner-a", 60)
            .unwrap(),
        ClaimStatus::Acquired
    );
    shard
        .complete_claim(
            b"jobs",
            b"finished",
            b"owner-a",
            b"result",
            b"fingerprint-a",
            b"value-a",
        )
        .unwrap();
    shard.close();

    let joined = dir.path().join("joined");
    ShardedConCorpus::open_existing(&root)
        .unwrap()
        .join_to_single_env(&joined)
        .unwrap();
    let joined = ConCorpus::open(&joined)
        .unwrap()
        .with_clock(TestClock::at(BASE_NS));
    assert_eq!(
        joined
            .try_claim(b"jobs", b"finished", b"owner-b", 60)
            .unwrap(),
        ClaimStatus::Complete
    );
    let records = joined.list_catalog_records(b"jobs", b"result").unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].fingerprint, b"fingerprint-a");
}

#[test]
fn live_lease_survives_split_with_unchanged_expiry() {
    let dir = tempfile::tempdir().unwrap();
    let clock = TestClock::at(BASE_NS);
    let single = ConCorpus::open(dir.path().join("single"))
        .unwrap()
        .with_clock(clock.clone());
    assert_eq!(
        single
            .try_claim(b"jobs", b"leased", b"owner-a", 30)
            .unwrap(),
        ClaimStatus::Acquired
    );

    let split = dir.path().join("split");
    ShardedConCorpus::split_single_to_sharded(&single, &split, 2).unwrap();
    let split_claims = ShardedConCorpus::open_shard(&split, 0)
        .unwrap()
        .with_clock(clock.clone());

    let expiry_ms = BASE_NS / NANOS_PER_MILLI + 30_001;
    clock.set(expiry_ms * NANOS_PER_MILLI);
    assert_eq!(
        split_claims
            .try_claim(b"jobs", b"leased", b"owner-b", 30)
            .unwrap(),
        ClaimStatus::Busy
    );
    clock.set((expiry_ms + 1) * NANOS_PER_MILLI);
    assert_eq!(
        split_claims
            .try_claim(b"jobs", b"leased", b"owner-b", 30)
            .unwrap(),
        ClaimStatus::Acquired
    );
}

#[test]
fn conflicting_claims_across_join_sources_error_before_dest_create() {
    let dir = tempfile::tempdir().unwrap();
    let source_a = dir.path().join("source-a");
    let source_b = dir.path().join("source-b");
    ShardedConCorpus::open(&source_a, 1).unwrap();
    ShardedConCorpus::open(&source_b, 1).unwrap();
    for (source, owner) in [(&source_a, b"owner-a".as_slice()), (&source_b, b"owner-b")] {
        let shard = ShardedConCorpus::open_shard(source, 0)
            .unwrap()
            .with_clock(TestClock::at(BASE_NS));
        assert_eq!(
            shard.try_claim(b"jobs", b"same-task", owner, 60).unwrap(),
            ClaimStatus::Acquired
        );
        shard.close();
    }

    let joined = dir.path().join("joined");
    let error = join_drained_roots(&[source_a, source_b], &joined).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("work claim conflict"), "{message}");
    assert!(message.contains("jobs"), "{message}");
    assert!(message.contains("same-task"), "{message}");
    assert!(!joined.exists());
}

#[test]
fn duplicate_catalog_identity_uses_same_refusal_policy_as_claims() {
    let dir = tempfile::tempdir().unwrap();
    let source_a = dir.path().join("source-a");
    let source_b = dir.path().join("source-b");
    ShardedConCorpus::open(&source_a, 1).unwrap();
    ShardedConCorpus::open(&source_b, 1).unwrap();
    for (source, value) in [(&source_a, b"value-a".as_slice()), (&source_b, b"value-b")] {
        ShardedConCorpus::open_shard(source, 0)
            .unwrap()
            .insert_catalog_record(b"jobs", b"result", b"same-fingerprint", value)
            .unwrap();
    }

    let joined = dir.path().join("joined");
    let error = join_drained_roots(&[source_a, source_b], &joined).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("catalog record conflict"), "{message}");
    assert!(message.contains("jobs"), "{message}");
    assert!(message.contains("result"), "{message}");
    assert!(message.contains("same-fingerprint"), "{message}");
    assert!(!joined.exists());
}

#[test]
fn split_homes_joined_claim_from_shard_one_in_shard_zero() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    ShardedConCorpus::open(&source, 2).unwrap();
    let shard_one = ShardedConCorpus::open_shard(&source, 1)
        .unwrap()
        .with_clock(TestClock::at(BASE_NS));
    assert_eq!(
        shard_one
            .try_claim(b"jobs", b"from-shard-one", b"owner-a", 30)
            .unwrap(),
        ClaimStatus::Acquired
    );
    let original_dump = shard_one.canonical_dump().unwrap();
    shard_one.close();

    let joined_path = dir.path().join("joined");
    ShardedConCorpus::open_existing(&source)
        .unwrap()
        .join_to_single_env(&joined_path)
        .unwrap();
    let joined = ConCorpus::open_readonly(&joined_path).unwrap();
    assert_eq!(
        joined.canonical_dump().unwrap(),
        original_dump,
        "join must preserve the encoded claim key and payload"
    );

    let split = dir.path().join("split");
    ShardedConCorpus::split_single_to_sharded(&joined, &split, 2).unwrap();
    joined.close();
    assert!(!split.join("shard_0001").exists());
    let shard_zero = ConCorpus::open_readonly(split.join("shard_0000")).unwrap();
    assert_eq!(
        shard_zero.canonical_dump().unwrap(),
        original_dump,
        "split must preserve the encoded claim key and payload in shard zero"
    );
}
