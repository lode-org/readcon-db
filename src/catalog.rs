//! Generic catalog records and atomic work claims stored in the corpus LMDB.

use heed::types::Bytes;
use heed::{Database, RwTxn};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::corpus::ConCorpus;
use crate::error::{ClaimError, Error, Result};

/// Source of the wall-clock instants that date work-claim leases.
///
/// Every corpus starts on [`SystemClock`], so leases follow the host clock
/// unless [`ConCorpus::with_clock`] replaces it.
pub trait Clock: Send + Sync {
    /// Nanoseconds since the unix epoch.
    fn unix_time_nanos(&self) -> Result<u128>;
}

/// The host wall clock, `SystemTime::now`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn unix_time_nanos(&self) -> Result<u128> {
        Ok(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Message("system clock is before unix epoch".into()))?
            .as_nanos())
    }
}

/// Maximum size of a namespace, subject, fingerprint, task, or owner.
pub const MAX_CATALOG_COMPONENT_BYTES: usize = 128;
/// Maximum size of one opaque catalog record.
pub const MAX_CATALOG_RECORD_BYTES: usize = 16 * 1024 * 1024;

const CLAIM_ACTIVE: u8 = 0;
const CLAIM_COMPLETE: u8 = 1;
const CLAIM_FORMAT_VERSION: u8 = 1;

/// One opaque catalog record and the identity under which it was deduplicated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogRecord {
    /// Caller-defined namespace.
    pub namespace: Vec<u8>,
    /// Caller-defined subject within the namespace.
    pub subject: Vec<u8>,
    /// Caller-defined content fingerprint.
    pub fingerprint: Vec<u8>,
    /// Opaque record bytes.
    pub value: Vec<u8>,
}

pub(crate) struct WorkClaimRecord {
    pub namespace: Vec<u8>,
    pub task: Vec<u8>,
    pub encoded_key: Vec<u8>,
    pub encoded_value: Vec<u8>,
}

pub(crate) struct CatalogState {
    pub records: Vec<CatalogRecord>,
    pub claims: Vec<WorkClaimRecord>,
}

/// Result of trying to claim a unit of work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimStatus {
    /// This call acquired the unclaimed task.
    Acquired,
    /// The task was already held by this owner.
    AlreadyOwned,
    /// A different owner currently holds the task.
    Busy,
    /// The task was previously completed.
    Complete,
}

fn validate_component(name: &str, value: &[u8]) -> Result<()> {
    if value.is_empty() {
        return Err(Error::Message(format!("{name} must not be empty")));
    }
    if value.len() > MAX_CATALOG_COMPONENT_BYTES {
        return Err(Error::Message(format!(
            "{name} is {} bytes; maximum is {MAX_CATALOG_COMPONENT_BYTES}",
            value.len()
        )));
    }
    Ok(())
}

fn validate_record(value: &[u8]) -> Result<()> {
    if value.is_empty() {
        return Err(Error::Message("catalog record must not be empty".into()));
    }
    if value.len() > MAX_CATALOG_RECORD_BYTES {
        return Err(Error::Message(format!(
            "catalog record is {} bytes; maximum is {MAX_CATALOG_RECORD_BYTES}",
            value.len()
        )));
    }
    Ok(())
}

fn push_component(key: &mut Vec<u8>, value: &[u8]) {
    key.extend_from_slice(&(value.len() as u16).to_be_bytes());
    key.extend_from_slice(value);
}

fn catalog_key(namespace: &[u8], subject: &[u8], fingerprint: &[u8]) -> Result<Vec<u8>> {
    validate_component("namespace", namespace)?;
    validate_component("subject", subject)?;
    validate_component("fingerprint", fingerprint)?;
    let mut key = Vec::with_capacity(6 + namespace.len() + subject.len() + fingerprint.len());
    push_component(&mut key, namespace);
    push_component(&mut key, subject);
    push_component(&mut key, fingerprint);
    Ok(key)
}

fn catalog_prefix(namespace: &[u8], subject: &[u8]) -> Result<Vec<u8>> {
    validate_component("namespace", namespace)?;
    validate_component("subject", subject)?;
    let mut key = Vec::with_capacity(4 + namespace.len() + subject.len());
    push_component(&mut key, namespace);
    push_component(&mut key, subject);
    Ok(key)
}

fn work_key(namespace: &[u8], task: &[u8]) -> Result<Vec<u8>> {
    validate_component("namespace", namespace)?;
    validate_component("task", task)?;
    let mut key = Vec::with_capacity(4 + namespace.len() + task.len());
    push_component(&mut key, namespace);
    push_component(&mut key, task);
    Ok(key)
}

fn take_component<'a>(encoded: &mut &'a [u8], what: &str) -> Result<&'a [u8]> {
    let len_bytes: [u8; 2] = encoded
        .get(..2)
        .ok_or_else(|| Error::Message(format!("corrupt {what}: missing component length")))?
        .try_into()
        .expect("slice length checked");
    let len = u16::from_be_bytes(len_bytes) as usize;
    let value = encoded
        .get(2..2 + len)
        .ok_or_else(|| Error::Message(format!("corrupt {what}: truncated component")))?;
    *encoded = &encoded[2 + len..];
    Ok(value)
}

fn decode_catalog_key(key: &[u8]) -> Result<(&[u8], &[u8], &[u8])> {
    let mut rest = key;
    let namespace = take_component(&mut rest, "catalog key")?;
    let subject = take_component(&mut rest, "catalog key")?;
    let fingerprint = take_component(&mut rest, "catalog key")?;
    if !rest.is_empty() {
        return Err(Error::Message("corrupt catalog key: trailing bytes".into()));
    }
    validate_component("stored namespace", namespace)?;
    validate_component("stored subject", subject)?;
    validate_component("stored fingerprint", fingerprint)?;
    Ok((namespace, subject, fingerprint))
}

fn decode_work_key(key: &[u8]) -> Result<(&[u8], &[u8])> {
    let mut rest = key;
    let namespace = take_component(&mut rest, "work claim key")?;
    let task = take_component(&mut rest, "work claim key")?;
    if !rest.is_empty() {
        return Err(Error::Message(
            "corrupt work claim key: trailing bytes".into(),
        ));
    }
    validate_component("stored namespace", namespace)?;
    validate_component("stored task", task)?;
    Ok((namespace, task))
}

fn quoted_component(value: &[u8]) -> String {
    format!("{:?}", String::from_utf8_lossy(value))
}

const NANOS_PER_MILLI: u128 = 1_000_000;
const MILLIS_PER_SEC: u64 = 1_000;

// The sample precedes the return (`t_s <= t_r`) because `sampled_expiry_ms`
// runs before `put` and before the write transaction commits. Stored
// expiries are whole milliseconds, and `ceil` adds nothing on a
// millisecond boundary. One quantum is the smallest constant that still
// gives `E >= t_r + L` while commit and return, after the sample, finish
// inside that same millisecond:
//
//   t_s <= t_r < t_s + 1ms
//   E = ceil_ms(t_s) + 1000*L + 1
//   E*1ms >= t_s + L + 1ms > t_r + L
//
// A slower commit can make `t_r >= t_s + 1ms`, and then `E` can be earlier
// than the return plus the lease. Takeover requires `expiry < floor_ms(now)`,
// so when the inequality holds it cannot happen before `(E + 1) ms`.
const LEASE_RETURN_MARGIN_MS: u64 = 1;

fn floor_unix_millis(now_ns: u128) -> Result<u64> {
    u64::try_from(now_ns / NANOS_PER_MILLI)
        .map_err(|_| Error::Message("system clock value overflowed u64 milliseconds".into()))
}

fn ceil_unix_millis(now_ns: u128) -> Result<u64> {
    u64::try_from(now_ns.div_ceil(NANOS_PER_MILLI))
        .map_err(|_| Error::Message("system clock value overflowed u64 milliseconds".into()))
}

fn lease_millis(lease_secs: u64) -> Result<u64> {
    if lease_secs == 0 {
        return Err(Error::Message(
            "lease_secs must be greater than zero".into(),
        ));
    }
    lease_secs
        .checked_mul(MILLIS_PER_SEC)
        .ok_or_else(|| Error::Message("lease duration overflowed u64 milliseconds".into()))
}

fn expiry_from_lease_secs(lease_ms: u64, now_ns: u128) -> Result<u64> {
    ceil_unix_millis(now_ns)?
        .checked_add(lease_ms)
        .and_then(|expiry| expiry.checked_add(LEASE_RETURN_MARGIN_MS))
        .ok_or_else(|| Error::Message("lease expiry overflowed u64 milliseconds".into()))
}

fn sampled_expiry_ms(clock: &dyn Clock, lease_ms: u64) -> Result<u64> {
    // Before put and commit. A post-commit sample cannot change the value
    // this transaction stores; covering a slower commit would be another write.
    expiry_from_lease_secs(lease_ms, clock.unix_time_nanos()?)
}

fn claim_value(state: u8, owner: &[u8], expiry_unix_ms: u64) -> Vec<u8> {
    let mut value = Vec::with_capacity(12 + owner.len());
    value.push(CLAIM_FORMAT_VERSION);
    value.push(state);
    value.extend_from_slice(&expiry_unix_ms.to_be_bytes());
    push_component(&mut value, owner);
    value
}

fn decode_claim(value: &[u8]) -> Result<(u8, u64, &[u8])> {
    let (&version, rest) = value
        .split_first()
        .ok_or_else(|| Error::Message("corrupt work claim: empty value".into()))?;
    if version != CLAIM_FORMAT_VERSION {
        return Err(ClaimError::UnknownClaimFormatVersion(version).into());
    }
    let (&state, mut rest) = rest
        .split_first()
        .ok_or_else(|| Error::Message("corrupt work claim: missing state".into()))?;
    if state != CLAIM_ACTIVE && state != CLAIM_COMPLETE {
        return Err(Error::Message(format!(
            "corrupt work claim: unknown state {state}"
        )));
    }
    let expiry_bytes: [u8; 8] = rest
        .get(..8)
        .ok_or_else(|| Error::Message("corrupt work claim: missing lease expiry".into()))?
        .try_into()
        .expect("slice length checked");
    let expiry_unix_ms = u64::from_be_bytes(expiry_bytes);
    rest = &rest[8..];
    let owner = take_component(&mut rest, "work claim")?;
    if !rest.is_empty() {
        return Err(Error::Message("corrupt work claim: trailing bytes".into()));
    }
    validate_component("stored claim owner", owner)?;
    Ok((state, expiry_unix_ms, owner))
}

fn insert_record(
    db: Database<Bytes, Bytes>,
    txn: &mut RwTxn<'_>,
    key: &[u8],
    value: &[u8],
) -> Result<bool> {
    if db.get(txn, key)?.is_some() {
        return Ok(false);
    }
    db.put(txn, key, value)?;
    Ok(true)
}

impl ConCorpus {
    /// Replace the wall clock that dates work-claim leases.
    ///
    /// Only the claim code reads it, and every constructor starts on
    /// [`SystemClock`]. A caller that needs an exact lease window, such as a
    /// test, installs a clock it advances itself.
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    fn writable_catalog_db(&self) -> Result<Database<Bytes, Bytes>> {
        if self.readonly {
            return Err(Error::Message(
                "catalog mutation is unavailable on a readonly corpus".into(),
            ));
        }
        self.catalog_records
            .ok_or_else(|| Error::Message("catalog database is unavailable".into()))
    }

    fn writable_claims_db(&self) -> Result<Database<Bytes, Bytes>> {
        if self.readonly {
            return Err(Error::Message(
                "work-claim mutation is unavailable on a readonly corpus".into(),
            ));
        }
        self.work_claims
            .ok_or_else(|| Error::Message("work-claims database is unavailable".into()))
    }

    /// Atomically insert an opaque record unless its
    /// `(namespace, subject, fingerprint)` identity already exists.
    ///
    /// Returns `true` when this call inserted the record and `false` when the
    /// identity was already present. An existing record is never overwritten.
    pub fn insert_catalog_record(
        &self,
        namespace: &[u8],
        subject: &[u8],
        fingerprint: &[u8],
        value: &[u8],
    ) -> Result<bool> {
        let key = catalog_key(namespace, subject, fingerprint)?;
        validate_record(value)?;
        let db = self.writable_catalog_db()?;
        self.with_write(value.len(), |txn| insert_record(db, txn, &key, value))
    }

    /// List records for exactly one `(namespace, subject)`.
    ///
    /// A readonly open of an older corpus without the catalog database returns
    /// an empty list.
    pub fn list_catalog_records(
        &self,
        namespace: &[u8],
        subject: &[u8],
    ) -> Result<Vec<CatalogRecord>> {
        let key_prefix = catalog_prefix(namespace, subject)?;
        let Some(db) = self.catalog_records else {
            return Ok(Vec::new());
        };
        let txn = self.read_txn()?;
        let mut records = Vec::new();
        for item in db.prefix_iter(&txn, &key_prefix)? {
            let (key, value) = item?;
            let (stored_namespace, stored_subject, fingerprint) = decode_catalog_key(key)?;
            records.push(CatalogRecord {
                namespace: stored_namespace.to_vec(),
                subject: stored_subject.to_vec(),
                fingerprint: fingerprint.to_vec(),
                value: value.to_vec(),
            });
        }
        Ok(records)
    }

    pub(crate) fn catalog_state(&self) -> Result<CatalogState> {
        let txn = self.read_txn()?;
        let mut records = Vec::new();
        if let Some(db) = self.catalog_records {
            for item in db.iter(&txn)? {
                let (key, value) = item?;
                let (namespace, subject, fingerprint) = decode_catalog_key(key)?;
                records.push(CatalogRecord {
                    namespace: namespace.to_vec(),
                    subject: subject.to_vec(),
                    fingerprint: fingerprint.to_vec(),
                    value: value.to_vec(),
                });
            }
        }
        let mut claims = Vec::new();
        if let Some(db) = self.work_claims {
            for item in db.iter(&txn)? {
                let (key, value) = item?;
                let (namespace, task) = decode_work_key(key)?;
                decode_claim(value)?;
                claims.push(WorkClaimRecord {
                    namespace: namespace.to_vec(),
                    task: task.to_vec(),
                    encoded_key: key.to_vec(),
                    encoded_value: value.to_vec(),
                });
            }
        }
        Ok(CatalogState { records, claims })
    }

    pub(crate) fn insert_copied_catalog_record(&self, record: &CatalogRecord) -> Result<()> {
        let key = catalog_key(&record.namespace, &record.subject, &record.fingerprint)?;
        validate_record(&record.value)?;
        let db = self.writable_catalog_db()?;
        self.with_write(record.value.len(), |txn| {
            if db.get(txn, &key)?.is_some() {
                return Err(Error::Message(format!(
                    "catalog record conflict: namespace {}, subject {}, fingerprint {} appears in more than one source",
                    quoted_component(&record.namespace),
                    quoted_component(&record.subject),
                    quoted_component(&record.fingerprint),
                )));
            }
            db.put(txn, &key, &record.value)?;
            Ok(())
        })
    }

    pub(crate) fn insert_copied_work_claim(&self, claim: &WorkClaimRecord) -> Result<()> {
        let (namespace, task) = decode_work_key(&claim.encoded_key)?;
        decode_claim(&claim.encoded_value)?;
        let db = self.writable_claims_db()?;
        self.with_write(claim.encoded_value.len(), |txn| {
            if db.get(txn, &claim.encoded_key)?.is_some() {
                return Err(Error::Message(format!(
                    "work claim conflict: namespace {}, key {} appears in more than one source",
                    quoted_component(namespace),
                    quoted_component(task),
                )));
            }
            db.put(txn, &claim.encoded_key, &claim.encoded_value)?;
            Ok(())
        })
    }

    /// Atomically try to claim one namespaced task for one owner.
    ///
    /// `lease_secs` is required and must be greater than zero.
    ///
    /// Lease timestamps use this corpus's [`Clock`] in milliseconds, which is
    /// the host wall clock unless [`Self::with_clock`] replaced it. They are
    /// evaluated only after this writer takes LMDB's write lock, so
    /// queueing behind another writer cannot create an already-expired lease.
    /// The clock is read immediately before the expiry is stored, which is
    /// before the row is put and before the write transaction commits. The
    /// stored millisecond is `ceil(sample)` plus the lease plus one
    /// millisecond. That is not earlier than this call's return plus
    /// `lease_secs` only while that commit and the return finish within one
    /// millisecond of the sample. A slower commit can leave the stored expiry
    /// earlier than the return plus the lease. A later caller takes the task
    /// over only when `expiry < now`.
    ///
    /// Clock steps affect lease lifetime directly: a backward step extends the
    /// observed lease and a forward step can expire it early. All participants
    /// must use one host clock. If an owner's lease has expired, that same
    /// owner gets `Acquired` when reclaiming the task.
    ///
    /// The `AlreadyOwned` result reports ownership as-is; it does **not** extend
    /// or renew the existing lease.
    pub fn try_claim(
        &self,
        namespace: &[u8],
        task: &[u8],
        owner: &[u8],
        lease_secs: u64,
    ) -> Result<ClaimStatus> {
        let key = work_key(namespace, task)?;
        validate_component("owner", owner)?;
        let db = self.writable_claims_db()?;
        self.with_write(16 + owner.len(), |txn| {
            // Reject a zero or overflowing lease before any read or put.
            // The clock sample stays after that check, immediately before
            // the deadline is sealed into the value.
            let lease_ms = lease_millis(lease_secs)?;
            let existing = db.get(txn, &key)?.map(|value| value.to_vec());
            match existing {
                None => {
                    let expiry_unix_ms = sampled_expiry_ms(self.clock.as_ref(), lease_ms)?;
                    let active = claim_value(CLAIM_ACTIVE, owner, expiry_unix_ms);
                    db.put(txn, &key, &active)?;
                    Ok(ClaimStatus::Acquired)
                }
                Some(value) => {
                    let (state, old_expiry_unix_ms, stored_owner) = decode_claim(&value)?;
                    let now_unix_ms = floor_unix_millis(self.clock.unix_time_nanos()?)?;
                    if state == CLAIM_COMPLETE {
                        Ok(ClaimStatus::Complete)
                    } else if old_expiry_unix_ms < now_unix_ms {
                        let expiry_unix_ms = sampled_expiry_ms(self.clock.as_ref(), lease_ms)?;
                        let active = claim_value(CLAIM_ACTIVE, owner, expiry_unix_ms);
                        db.put(txn, &key, &active)?;
                        Ok(ClaimStatus::Acquired)
                    } else if stored_owner == owner {
                        Ok(ClaimStatus::AlreadyOwned)
                    } else {
                        Ok(ClaimStatus::Busy)
                    }
                }
            }
        })
    }

    /// Renew an active claim lease for the current owner.
    ///
    /// Lease timestamps use this corpus's [`Clock`] in milliseconds and are
    /// computed after this writer holds LMDB's write lock. The stored
    /// expiry uses the same `ceil(sample) + lease + 1ms` deadline as
    /// [`Self::try_claim`]. As there, the sample is taken before put and
    /// commit, so the expiry is not earlier than this call's return plus
    /// `lease_secs` only while commit and return finish within one millisecond
    /// of the sample.
    ///
    /// Clock steps are observed as-is: a backward step extends the lease and a
    /// forward step can make it immediately expired. All claimants should run
    /// against one shared host clock.
    pub fn renew_claim(
        &self,
        namespace: &[u8],
        task: &[u8],
        owner: &[u8],
        lease_secs: u64,
    ) -> Result<()> {
        let key = work_key(namespace, task)?;
        validate_component("owner", owner)?;
        let db = self.writable_claims_db()?;
        self.with_write(16 + owner.len(), |txn| {
            let lease_ms = lease_millis(lease_secs)?;
            let claim = db.get(txn, &key)?.ok_or(ClaimError::Unclaimed)?.to_vec();
            let (state, old_expiry_unix_ms, stored_owner) = decode_claim(&claim)?;
            if state == CLAIM_COMPLETE {
                return Err(ClaimError::AlreadyComplete.into());
            }
            if stored_owner != owner {
                return Err(ClaimError::NotOwner.into());
            }
            let now_unix_ms = floor_unix_millis(self.clock.unix_time_nanos()?)?;
            if old_expiry_unix_ms < now_unix_ms {
                return Err(ClaimError::Expired.into());
            }
            // Resample immediately before the put so the stored deadline is
            // the latest instant this transaction can still record.
            let expiry_unix_ms = sampled_expiry_ms(self.clock.as_ref(), lease_ms)?;
            let renewed = claim_value(CLAIM_ACTIVE, owner, expiry_unix_ms);
            db.put(txn, &key, &renewed)?;
            Ok(())
        })
    }

    /// Atomically insert a catalog record and mark a claimed task complete.
    ///
    /// Only the active owner may complete the task. Returns whether this call
    /// inserted the record; a matching record that already exists is retained.
    ///
    /// Uses the same wall clock semantics as [`Self::try_claim`].
    pub fn complete_claim(
        &self,
        namespace: &[u8],
        task: &[u8],
        owner: &[u8],
        subject: &[u8],
        fingerprint: &[u8],
        value: &[u8],
    ) -> Result<bool> {
        let work_key = work_key(namespace, task)?;
        validate_component("owner", owner)?;
        let record_key = catalog_key(namespace, subject, fingerprint)?;
        validate_record(value)?;
        let claims = self.writable_claims_db()?;
        let records = self.writable_catalog_db()?;
        let complete = claim_value(CLAIM_COMPLETE, owner, 0);
        self.with_write(value.len().saturating_add(complete.len()), |txn| {
            let claim = claims
                .get(txn, &work_key)?
                .ok_or(ClaimError::Unclaimed)?
                .to_vec();
            let (state, expiry_unix_ms, stored_owner) = decode_claim(&claim)?;
            let now_unix_ms = floor_unix_millis(self.clock.unix_time_nanos()?)?;
            if state == CLAIM_COMPLETE {
                return Err(ClaimError::AlreadyComplete.into());
            }
            if stored_owner != owner {
                return Err(ClaimError::NotOwner.into());
            }
            if expiry_unix_ms < now_unix_ms {
                return Err(ClaimError::Expired.into());
            }
            let inserted = insert_record(records, txn, &record_key, value)?;
            claims.put(txn, &work_key, &complete)?;
            Ok(inserted)
        })
    }

    /// Atomically release an active claim.
    ///
    /// Only the current owner may release it. Completed work cannot be
    /// released.
    ///
    /// Release is owner-checked but ignores expiry: even a lapsed owner may
    /// clear their stale claim record.
    pub fn release_claim(&self, namespace: &[u8], task: &[u8], owner: &[u8]) -> Result<()> {
        let key = work_key(namespace, task)?;
        validate_component("owner", owner)?;
        let db = self.writable_claims_db()?;
        self.with_write(0, |txn| {
            let claim = db.get(txn, &key)?.ok_or(ClaimError::Unclaimed)?;
            let (state, _expiry_unix_ms, stored_owner) = decode_claim(claim)?;
            if state == CLAIM_COMPLETE {
                return Err(ClaimError::AlreadyComplete.into());
            }
            if stored_owner != owner {
                return Err(ClaimError::NotOwner.into());
            }
            db.delete(txn, &key)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    /// Keep sampling until one call returns inside the 1ms margin, or this
    /// elapses. A scheduler bound. Zero eligible iterations is a failure.
    const STORED_EXPIRY_RETRY_LIMIT: Duration = Duration::from_secs(5);

    fn stored_expiry_ms(corpus: &ConCorpus, namespace: &[u8], task: &[u8]) -> u64 {
        let key = work_key(namespace, task).unwrap();
        let db = corpus.writable_claims_db().unwrap();
        let txn = corpus.read_txn().unwrap();
        let value = db.get(&txn, &key).unwrap().unwrap();
        let (_state, expiry_ms, _owner) = decode_claim(value).unwrap();
        expiry_ms
    }

    fn expiry_covers_return(expiry_ms: u64, returned: SystemTime, lease_secs: u64) -> bool {
        let expiry_ns = u128::from(expiry_ms) * 1_000_000;
        let returned_ns = returned.duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let lease_ns = u128::from(lease_secs) * 1_000_000_000;
        expiry_ns >= returned_ns + lease_ns
    }

    fn returned_within_margin(
        before: Instant,
        before_wall: SystemTime,
        returned: SystemTime,
    ) -> bool {
        before.elapsed() < Duration::from_millis(1)
            && returned
                .duration_since(before_wall)
                .map(|gap| gap < Duration::from_millis(1))
                .unwrap_or(false)
    }

    /// `floor(sample) + lease + 1ms`. Rejected: a sub-ms sample stays on the
    /// previous millisecond, so the margin does not cover that quantum.
    fn floor_plus_margin(lease_ms: u64, now_ns: u128) -> u64 {
        u64::try_from(now_ns / 1_000_000).unwrap() + lease_ms + 1
    }

    /// `ceil(sample) + lease`, with no return margin. Rejected: on a
    /// millisecond boundary ceil adds nothing, and the stored quantum is short.
    fn ceil_no_margin(lease_ms: u64, now_ns: u128) -> u64 {
        u64::try_from(now_ns.div_ceil(1_000_000)).unwrap() + lease_ms
    }

    #[test]
    fn expiry_from_lease_secs_ceils_sub_ms_and_adds_one_ms_margin() {
        // Arguments are `(lease_ms, now_ns)` on this signature. The same
        // numerals are not an r4 call: r4 takes `(lease_secs, now_ms)`.
        let samples: &[(u64, u128)] = &[
            (1_000, 1),
            (1_000, 999_999),
            (1_000, 1_000_000),
            (1_000, 1_000_001),
            (1_000, 1_500_000),
            (2_000, 1_500_000),
        ];
        let mut floor_margin_rejected = false;
        let mut ceil_plain_rejected = false;
        for &(lease_ms, now_ns) in samples {
            let got = expiry_from_lease_secs(lease_ms, now_ns).unwrap();
            let ceil_ms = u64::try_from(now_ns.div_ceil(1_000_000)).unwrap();
            assert_eq!(got, ceil_ms + lease_ms + 1);
            if got != floor_plus_margin(lease_ms, now_ns) {
                floor_margin_rejected = true;
            }
            if got != ceil_no_margin(lease_ms, now_ns) {
                ceil_plain_rejected = true;
            }
        }
        assert!(floor_margin_rejected, "floor+margin matched every sample");
        assert!(
            ceil_plain_rejected,
            "ceil without the 1ms margin matched every sample"
        );
    }

    #[test]
    fn try_claim_stored_expiry_covers_return_within_margin() {
        let dir = tempfile::tempdir().unwrap();
        let corpus = ConCorpus::open(dir.path()).unwrap();
        let started = Instant::now();
        let mut eligible = 0u32;
        let mut trial = 0u32;
        while started.elapsed() < STORED_EXPIRY_RETRY_LIMIT && eligible < 1 {
            trial += 1;
            let task = format!("fast-expiry-{trial}");
            let before = Instant::now();
            let before_wall = SystemTime::now();
            assert_eq!(
                corpus
                    .try_claim(b"jobs", task.as_bytes(), b"owner-a", 1)
                    .unwrap(),
                ClaimStatus::Acquired
            );
            let returned = SystemTime::now();
            if !returned_within_margin(before, before_wall, returned) {
                continue;
            }
            eligible += 1;
            let expiry_ms = stored_expiry_ms(&corpus, b"jobs", task.as_bytes());
            assert!(
                expiry_covers_return(expiry_ms, returned, 1),
                "trial {trial}: stored expiry {expiry_ms} ms is earlier than return+1s ({returned:?})"
            );
        }
        println!(
            "try_claim_stored_expiry_covers_return_within_margin asserting_iterations={eligible}"
        );
        assert!(
            eligible >= 1,
            "no try_claim call returned within the 1ms margin ({trial} trials)"
        );
    }

    #[test]
    fn renew_claim_stored_expiry_covers_return_plus_lease() {
        let dir = tempfile::tempdir().unwrap();
        let corpus = ConCorpus::open(dir.path()).unwrap();
        let namespace = b"jobs";
        let task = b"task-renew-expiry";
        assert_eq!(
            corpus.try_claim(namespace, task, b"owner-a", 30).unwrap(),
            ClaimStatus::Acquired
        );
        let started = Instant::now();
        let mut eligible = 0u32;
        let mut trial = 0u32;
        while started.elapsed() < STORED_EXPIRY_RETRY_LIMIT && eligible < 1 {
            trial += 1;
            let before = Instant::now();
            let before_wall = SystemTime::now();
            corpus.renew_claim(namespace, task, b"owner-a", 1).unwrap();
            let returned = SystemTime::now();
            if !returned_within_margin(before, before_wall, returned) {
                continue;
            }
            eligible += 1;
            let expiry_ms = stored_expiry_ms(&corpus, namespace, task);
            assert!(
                expiry_covers_return(expiry_ms, returned, 1),
                "trial {trial}: stored expiry {expiry_ms} ms is earlier than return+1s ({returned:?})"
            );
        }
        println!(
            "renew_claim_stored_expiry_covers_return_plus_lease asserting_iterations={eligible}"
        );
        assert!(
            eligible >= 1,
            "no renew_claim call returned within the 1ms margin ({trial} trials)"
        );
    }

    #[test]
    fn try_claim_computes_lease_after_writer_lock_is_taken() {
        let dir = tempfile::tempdir().unwrap();
        let corpus = Arc::new(ConCorpus::open(dir.path()).unwrap());
        let (held_tx, held_rx) = mpsc::channel();
        let lock_holder = {
            let corpus = Arc::clone(&corpus);
            thread::spawn(move || {
                let db = corpus.writable_claims_db().unwrap();
                corpus
                    .with_write(0, |_txn| {
                        // touch the claims DB inside this write txn
                        let _ = db;
                        held_tx.send(()).unwrap();
                        thread::sleep(Duration::from_secs(3));
                        Ok(())
                    })
                    .unwrap();
            })
        };
        held_rx.recv().unwrap();
        let start = Arc::new(Barrier::new(3));
        let a = {
            let corpus = Arc::clone(&corpus);
            let start = Arc::clone(&start);
            thread::spawn(move || {
                start.wait();
                corpus
                    .try_claim(b"jobs", b"task-race", b"owner-a", 2)
                    .unwrap()
            })
        };
        let b = {
            let corpus = Arc::clone(&corpus);
            let start = Arc::clone(&start);
            thread::spawn(move || {
                start.wait();
                corpus
                    .try_claim(b"jobs", b"task-race", b"owner-b", 2)
                    .unwrap()
            })
        };
        start.wait();
        let statuses = [a.join().unwrap(), b.join().unwrap()];
        lock_holder.join().unwrap();
        assert_eq!(
            statuses
                .iter()
                .filter(|status| **status == ClaimStatus::Acquired)
                .count(),
            1,
            "only one owner should acquire while the lease is still fresh",
        );
    }

    #[test]
    fn renew_claim_must_not_write_an_already_expired_lease() {
        let dir = tempfile::tempdir().unwrap();
        let corpus = Arc::new(ConCorpus::open(dir.path()).unwrap());
        assert_eq!(
            corpus
                .try_claim(b"jobs", b"task-renew", b"owner-a", 30)
                .unwrap(),
            ClaimStatus::Acquired
        );
        let (held_tx, held_rx) = mpsc::channel();
        let lock_holder = {
            let corpus = Arc::clone(&corpus);
            thread::spawn(move || {
                let db = corpus.writable_claims_db().unwrap();
                corpus
                    .with_write(0, |_txn| {
                        let _ = db;
                        held_tx.send(()).unwrap();
                        thread::sleep(Duration::from_secs(3));
                        Ok(())
                    })
                    .unwrap();
            })
        };
        held_rx.recv().unwrap();
        let renewer = {
            let corpus = Arc::clone(&corpus);
            thread::spawn(move || {
                corpus
                    .renew_claim(b"jobs", b"task-renew", b"owner-a", 2)
                    .unwrap();
            })
        };
        renewer.join().unwrap();
        lock_holder.join().unwrap();
        assert_eq!(
            corpus
                .try_claim(b"jobs", b"task-renew", b"owner-b", 30)
                .unwrap(),
            ClaimStatus::Busy,
            "renew should extend from the lock-acquired time, not from call entry",
        );
    }

    #[test]
    fn unknown_claim_format_version_is_typed_error() {
        let dir = tempfile::tempdir().unwrap();
        let corpus = ConCorpus::open(dir.path()).unwrap();
        assert_eq!(
            corpus
                .try_claim(b"jobs", b"task-version", b"owner-a", 30)
                .unwrap(),
            ClaimStatus::Acquired
        );
        let key = work_key(b"jobs", b"task-version").unwrap();
        let claims = corpus.writable_claims_db().unwrap();
        corpus
            .with_write(0, |txn| {
                let original = claims.get(txn, &key)?.unwrap().to_vec();
                let mut bad = original;
                bad[0] = 99;
                claims.put(txn, &key, &bad)?;
                Ok(())
            })
            .unwrap();
        let err = corpus
            .try_claim(b"jobs", b"task-version", b"owner-a", 30)
            .unwrap_err();
        assert!(matches!(
            err,
            Error::Claim(ClaimError::UnknownClaimFormatVersion(99))
        ));
    }
}
