"""Unit tests for the lockfile singleton (_claim_singleton)."""
import os

import main


def test_claim_writes_own_pid_when_no_lockfile(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(main, "_terminate_pid", lambda pid: called.append(pid))
    monkeypatch.setattr(main, "_is_process_alive", lambda pid: False)
    lock = tmp_path / "dialogue_reader.lock"
    main._claim_singleton(lock)
    assert lock.read_text(encoding="utf-8").strip() == str(os.getpid())
    assert called == []


def test_claim_terminates_live_prior_pid(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(main, "_terminate_pid", lambda pid: called.append(pid))
    monkeypatch.setattr(main, "_is_process_alive", lambda pid: True)
    monkeypatch.setattr(main.time, "sleep", lambda s: None)
    lock = tmp_path / "dialogue_reader.lock"
    lock.write_text("424242", encoding="utf-8")
    main._claim_singleton(lock)
    assert called == [424242]
    assert lock.read_text(encoding="utf-8").strip() == str(os.getpid())


def test_claim_skips_dead_prior_pid(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(main, "_terminate_pid", lambda pid: called.append(pid))
    monkeypatch.setattr(main, "_is_process_alive", lambda pid: False)
    lock = tmp_path / "dialogue_reader.lock"
    lock.write_text("424242", encoding="utf-8")
    main._claim_singleton(lock)
    assert called == []
    assert lock.read_text(encoding="utf-8").strip() == str(os.getpid())


def test_claim_ignores_garbage_lockfile(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(main, "_terminate_pid", lambda pid: called.append(pid))
    monkeypatch.setattr(main, "_is_process_alive", lambda pid: True)
    lock = tmp_path / "dialogue_reader.lock"
    lock.write_text("not-a-pid", encoding="utf-8")
    main._claim_singleton(lock)
    assert called == []
    assert lock.read_text(encoding="utf-8").strip() == str(os.getpid())


def test_claim_does_not_self_terminate(tmp_path, monkeypatch):
    called = []
    monkeypatch.setattr(main, "_terminate_pid", lambda pid: called.append(pid))
    monkeypatch.setattr(main, "_is_process_alive", lambda pid: True)
    lock = tmp_path / "dialogue_reader.lock"
    lock.write_text(str(os.getpid()), encoding="utf-8")
    main._claim_singleton(lock)
    assert called == []
    assert lock.read_text(encoding="utf-8").strip() == str(os.getpid())
