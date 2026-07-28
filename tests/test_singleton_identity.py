"""The singleton lock must prove the PID it kills is really a prior reader.

The lock file survives reboots and hard kills, and Windows recycles PIDs, so a
bare PID is not identity: launching the reader could TerminateProcess whatever
happens to own that number now (issue #22).
"""
import json
import os

import main as main_mod
from main import _claim_singleton, _release_singleton


def _lock(tmp_path):
    return tmp_path / "dialogue_reader.lock"


def _spy(monkeypatch):
    killed = []
    monkeypatch.setattr(main_mod, "_terminate_pid", lambda pid: killed.append(pid))
    monkeypatch.setattr(main_mod.time, "sleep", lambda s: None)
    return killed


def _write(path, **fields):
    path.write_text(json.dumps(fields), encoding="utf-8")


def _identity(pid=4321, name="python.exe", create_time=111.0):
    return {"pid": pid, "name": name, "create_time": create_time}


def _fake_psutil(monkeypatch, procs):
    """procs: {pid: (name, create_time)} for processes that exist."""
    class FakeProc:
        def __init__(self, pid):
            if pid not in procs:
                raise main_mod.psutil.NoSuchProcess(pid)
            self._n, self._c = procs[pid]

        def name(self):
            return self._n

        def create_time(self):
            return self._c

    monkeypatch.setattr(main_mod.psutil, "Process", FakeProc)
    monkeypatch.setattr(main_mod, "_is_process_alive", lambda pid: pid in procs)


# ---- identity checks -------------------------------------------------------

def test_kills_a_matching_prior_reader(tmp_path, monkeypatch):
    killed = _spy(monkeypatch)
    p = _lock(tmp_path)
    _write(p, **_identity())
    _fake_psutil(monkeypatch, {4321: ("python.exe", 111.0)})
    _claim_singleton(p)
    assert killed == [4321]


def test_recycled_pid_with_different_create_time_is_spared(tmp_path, monkeypatch):
    # Same PID number, but the process started later: it is NOT our old reader.
    killed = _spy(monkeypatch)
    p = _lock(tmp_path)
    _write(p, **_identity(create_time=111.0))
    _fake_psutil(monkeypatch, {4321: ("python.exe", 999.0)})
    _claim_singleton(p)
    assert killed == []


def test_recycled_pid_with_different_image_is_spared(tmp_path, monkeypatch):
    killed = _spy(monkeypatch)
    p = _lock(tmp_path)
    _write(p, **_identity(name="python.exe"))
    _fake_psutil(monkeypatch, {4321: ("Vein-Win64-Test.exe", 111.0)})
    _claim_singleton(p)
    assert killed == []


def test_legacy_bare_pid_lock_never_kills(tmp_path, monkeypatch):
    # Pre-upgrade lock files carry no identity, so they cannot be trusted.
    killed = _spy(monkeypatch)
    p = _lock(tmp_path)
    p.write_text("4321", encoding="utf-8")
    _fake_psutil(monkeypatch, {4321: ("python.exe", 111.0)})
    _claim_singleton(p)
    assert killed == []


def test_dead_pid_is_not_killed(tmp_path, monkeypatch):
    killed = _spy(monkeypatch)
    p = _lock(tmp_path)
    _write(p, **_identity())
    _fake_psutil(monkeypatch, {})
    _claim_singleton(p)
    assert killed == []


def test_missing_or_garbage_lock_is_tolerated(tmp_path, monkeypatch):
    killed = _spy(monkeypatch)
    _fake_psutil(monkeypatch, {})
    _claim_singleton(_lock(tmp_path))              # missing
    garbage = tmp_path / "g.lock"
    garbage.write_text("{not json", encoding="utf-8")
    _claim_singleton(garbage)
    assert killed == []


# ---- lock lifecycle --------------------------------------------------------

def test_claim_writes_our_identity(tmp_path, monkeypatch):
    _spy(monkeypatch)
    _fake_psutil(monkeypatch, {os.getpid(): ("python.exe", 555.0)})
    p = _lock(tmp_path)
    _claim_singleton(p)
    rec = json.loads(p.read_text(encoding="utf-8"))
    assert rec["pid"] == os.getpid()
    assert rec["name"] == "python.exe"
    assert rec["create_time"] == 555.0


def test_claim_never_self_terminates(tmp_path, monkeypatch):
    killed = _spy(monkeypatch)
    p = _lock(tmp_path)
    _write(p, **_identity(pid=os.getpid()))
    _fake_psutil(monkeypatch, {os.getpid(): ("python.exe", 111.0)})
    _claim_singleton(p)
    assert killed == []


def test_release_removes_only_our_own_lock(tmp_path, monkeypatch):
    _spy(monkeypatch)
    _fake_psutil(monkeypatch, {})
    p = _lock(tmp_path)
    _claim_singleton(p)
    _release_singleton(p)
    assert not p.exists()
    # A lock claimed by a DIFFERENT pid must survive our release.
    _write(p, **_identity(pid=99999))
    _release_singleton(p)
    assert p.exists()
