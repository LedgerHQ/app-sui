"""
Tee Speculos emulator stderr into an in-memory buffer so tests can assert on firmware logs.

Speculos is started via subprocess.Popen before individual tests run, so pytest's capfd
does not see emulator [ERROR] lines. We wrap Popen only for `python -m speculos` invocations.
"""
from __future__ import annotations

import subprocess
import sys
import threading
from typing import List, Optional

_chunks: List[bytes] = []
_max_bytes = 2 * 1024 * 1024
_current_size = 0
_orig_popen: Optional[type] = None
_lock = threading.Lock()


def clear() -> None:
    global _current_size
    with _lock:
        _chunks.clear()
        _current_size = 0


def text() -> str:
    with _lock:
        data = b"".join(_chunks)
    return data.decode("utf-8", errors="replace")


def _append_chunk(chunk: bytes) -> None:
    global _current_size
    with _lock:
        if _current_size >= _max_bytes:
            return
        if _current_size + len(chunk) > _max_bytes:
            chunk = chunk[: _max_bytes - _current_size]
        _chunks.append(chunk)
        _current_size += len(chunk)


def _is_speculos_launch(cmd: object) -> bool:
    if not isinstance(cmd, (list, tuple)) or len(cmd) < 3:
        return False
    return cmd[1] == "-m" and cmd[2] == "speculos"


def _tee_thread(read_fd: int) -> None:
    try:
        with open(read_fd, "rb", buffering=0, closefd=True) as f:
            while True:
                chunk = f.read(4096)
                if not chunk:
                    break
                _append_chunk(chunk)
                try:
                    sys.__stderr__.buffer.write(chunk)
                    sys.__stderr__.buffer.flush()
                except OSError:
                    pass
    finally:
        pass


def _popen(*a: object, **kw: object):
    cmd = a[0] if a else kw.get("args")  # type: ignore[assignment]
    if _is_speculos_launch(cmd) and kw.get("stderr", None) is None:
        import os

        kw = dict(kw)
        read_fd, write_fd = os.pipe()
        kw["stderr"] = write_fd
        proc = _orig_popen(*a, **kw)
        os.close(write_fd)
        t = threading.Thread(target=_tee_thread, args=(read_fd,), daemon=True)
        t.start()
        return proc
    return _orig_popen(*a, **kw)


def install() -> None:
    global _orig_popen
    if _orig_popen is not None:
        return
    _orig_popen = subprocess.Popen
    subprocess.Popen = _popen  # type: ignore[assignment]


def uninstall() -> None:
    global _orig_popen
    if _orig_popen is None:
        return
    subprocess.Popen = _orig_popen
    _orig_popen = None
