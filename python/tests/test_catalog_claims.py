"""Catalog claim behavior and typed Python exceptions."""

import multiprocessing
import time

import pytest
import readcon_db
from readcon_db import ConCorpus


def _process_try_claim(path, owner, start_event, out_queue):
    db = ConCorpus(path)
    start_event.wait()
    out_queue.put((owner, db.try_claim(b"ns", b"task-race", owner, 30)))


def test_try_claim_and_renew_with_lease(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    assert hasattr(db, "renew_claim"), "renew_claim must exist for leased claims"
    assert db.try_claim(b"ns", b"task", b"owner-a", 30) == "Acquired"
    assert db.try_claim(b"ns", b"task", b"owner-a", 30) == "AlreadyOwned"
    db.renew_claim(b"ns", b"task", b"owner-a", 30)
    assert db.try_claim(b"ns", b"task", b"owner-b", 30) == "Busy"


def test_typed_claim_exceptions(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    assert hasattr(readcon_db, "ClaimUnclaimedError")
    assert hasattr(readcon_db, "ClaimNotOwnerError")
    assert hasattr(readcon_db, "ClaimAlreadyCompleteError")
    assert hasattr(readcon_db, "ClaimExpiredError")
    assert hasattr(readcon_db, "ClaimError")
    ClaimUnclaimedError = readcon_db.ClaimUnclaimedError
    ClaimNotOwnerError = readcon_db.ClaimNotOwnerError
    ClaimAlreadyCompleteError = readcon_db.ClaimAlreadyCompleteError
    ClaimExpiredError = readcon_db.ClaimExpiredError
    ClaimError = readcon_db.ClaimError
    assert issubclass(ClaimError, RuntimeError)
    assert issubclass(ClaimUnclaimedError, ClaimError)
    assert issubclass(ClaimNotOwnerError, ClaimError)
    assert issubclass(ClaimAlreadyCompleteError, ClaimError)
    assert issubclass(ClaimExpiredError, ClaimError)

    try:
        db.release_claim(b"ns", b"missing", b"owner-a")
        raise AssertionError("release on unclaimed task must fail")
    except ClaimUnclaimedError:
        pass

    assert db.try_claim(b"ns", b"task", b"owner-a", 30) == "Acquired"

    try:
        db.renew_claim(b"ns", b"task", b"owner-b", 30)
        raise AssertionError("renew by non-owner must fail")
    except ClaimNotOwnerError:
        pass

    assert db.complete_claim(b"ns", b"task", b"owner-a", b"subject", b"fp", b"value")
    try:
        db.release_claim(b"ns", b"task", b"owner-a")
        raise AssertionError("release after completion must fail")
    except ClaimAlreadyCompleteError:
        pass


def test_expired_claim_is_reacquired_and_old_owner_cannot_complete(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    ClaimNotOwnerError = readcon_db.ClaimNotOwnerError
    ClaimExpiredError = readcon_db.ClaimExpiredError
    assert db.try_claim(b"ns", b"task", b"owner-a", 1) == "Acquired"
    time.sleep(2.0)
    assert db.try_claim(b"ns", b"task", b"owner-b", 30) == "Acquired"

    try:
        db.complete_claim(b"ns", b"task", b"owner-a", b"subject", b"fp", b"value")
        raise AssertionError("lapsed owner should not complete task")
    except ClaimNotOwnerError:
        pass

    assert db.try_claim(b"ns", b"expiring", b"owner-a", 1) == "Acquired"
    time.sleep(2.0)
    try:
        db.renew_claim(b"ns", b"expiring", b"owner-a", 30)
        raise AssertionError("expired claim should not be renewable")
    except ClaimExpiredError:
        pass


def test_zero_lease_is_rejected(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    with pytest.raises(RuntimeError, match="lease_secs must be greater than zero"):
        db.try_claim(b"ns", b"task", b"owner-a", 0)


def test_complete_expired_claim_before_takeover_fails_as_expired(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    assert db.try_claim(b"ns", b"task", b"owner-a", 1) == "Acquired"
    time.sleep(2.0)
    with pytest.raises(readcon_db.ClaimExpiredError):
        db.complete_claim(b"ns", b"task", b"owner-a", b"subject", b"fp", b"value")


def test_owner_reclaims_its_own_expired_task(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    assert db.try_claim(b"ns", b"task", b"owner-a", 1) == "Acquired"
    time.sleep(2.0)
    assert db.try_claim(b"ns", b"task", b"owner-a", 30) == "Acquired"


def test_claim_race_across_processes(tmp_path):
    corpus_path = str(tmp_path / "corpus")
    ConCorpus(corpus_path)
    ctx = multiprocessing.get_context("fork")
    start_event = ctx.Event()
    out_queue = ctx.Queue()
    owners = [b"owner-a", b"owner-b"]
    procs = [
        ctx.Process(
            target=_process_try_claim,
            args=(corpus_path, owner, start_event, out_queue),
        )
        for owner in owners
    ]
    for proc in procs:
        proc.start()
    start_event.set()
    results = [out_queue.get(timeout=10) for _ in owners]
    for proc in procs:
        proc.join(timeout=10)
        assert proc.exitcode == 0
    statuses = sorted(status for _, status in results)
    assert statuses == ["Acquired", "Busy"]
