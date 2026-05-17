"""macOS runtime bootstrap for PyGObject/Homebrew GTK."""

from __future__ import annotations

import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path


_REEXEC_MARKER = "DICTAITE_GTK_BOOTSTRAPPED"


def ensure_macos_gtk_paths(module_name: str) -> None:
    """Re-exec with Homebrew GTK paths before importing ``gi.repository``.

    PyGObject wheels can be installed into the virtualenv, but GTK/GLib remain
    native libraries. On macOS, dyld needs ``DYLD_LIBRARY_PATH`` at process
    startup to resolve typelib references such as ``libglib-2.0.0.dylib``.
    """

    if platform.system() != "Darwin" or os.environ.get(_REEXEC_MARKER):
        return

    prefixes = _homebrew_prefixes()
    changed = False
    env = os.environ.copy()
    for prefix in prefixes:
        changed |= _prepend_existing(env, "DYLD_LIBRARY_PATH", prefix / "lib")
        changed |= _prepend_existing(env, "GI_TYPELIB_PATH", prefix / "lib" / "girepository-1.0")
        changed |= _prepend_existing(env, "XDG_DATA_DIRS", prefix / "share")

    if not changed:
        return

    env[_REEXEC_MARKER] = "1"
    os.execvpe(sys.executable, [sys.executable, "-m", module_name, *sys.argv[1:]], env)


def _homebrew_prefixes() -> list[Path]:
    prefixes = [Path("/opt/homebrew"), Path("/usr/local")]

    brew = shutil.which("brew")
    if brew:
        try:
            output = subprocess.check_output([brew, "--prefix"], text=True, stderr=subprocess.DEVNULL).strip()
        except (OSError, subprocess.SubprocessError):
            output = ""
        if output:
            prefixes.insert(0, Path(output))

    deduped: list[Path] = []
    for prefix in prefixes:
        if prefix not in deduped:
            deduped.append(prefix)
    return deduped


def _prepend_existing(env: dict[str, str], name: str, path: Path) -> bool:
    if not path.exists():
        return False

    value = str(path)
    current = [part for part in env.get(name, "").split(os.pathsep) if part]
    if value in current:
        return False

    env[name] = os.pathsep.join([value, *current])
    return True
