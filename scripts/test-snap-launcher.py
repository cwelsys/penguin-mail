#!/usr/bin/env python3
"""Check the snap launcher's library lookup without the host's loader cache."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class LauncherTests(unittest.TestCase):
    def test_libproxy_finds_its_private_backend(self):
        compiler = shutil.which("cc")
        loader = Path("/lib64/ld-linux-x86-64.so.2")
        if not compiler or not loader.exists():
            self.skipTest("needs a C compiler and the x86-64 glibc loader")
        launcher = Path(__file__).resolve().parents[1] / "snap/local/launcher"
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            libraries = root / "usr/lib/x86_64-linux-gnu"
            backend = libraries / "libproxy"
            backend.mkdir(parents=True)
            for name in ("home", "data", "common"):
                (root / name).mkdir()
            (root / "backend.c").write_text("int backend(void) { return 42; }\n")
            (root / "proxy.c").write_text(
                "extern int backend(void); int proxy(void) { return backend(); }\n"
            )
            (root / "probe.c").write_text(
                "extern int proxy(void); int main(void) { return proxy() != 42; }\n"
            )
            commands = [
                [compiler, "-shared", "-fPIC", "backend.c", "-Wl,-soname,libpxbackend-1.0.so",
                 "-o", str(backend / "libpxbackend-1.0.so")],
                [compiler, "-shared", "-fPIC", "proxy.c", f"-L{backend}",
                 "-l:libpxbackend-1.0.so", "-Wl,-soname,libproxy.so.1",
                 "-o", str(libraries / "libproxy.so.1")],
                [compiler, "probe.c", f"-L{libraries}", "-l:libproxy.so.1",
                 f"-Wl,-rpath-link,{backend}", "-o", "probe"],
            ]
            for command in commands:
                subprocess.run(command, cwd=root, check=True, capture_output=True)
            environment = dict(os.environ)
            environment.pop("LD_LIBRARY_PATH", None)
            environment.update(
                SNAP=str(root), SNAP_USER_COMMON=str(root / "common"),
                SNAP_USER_DATA=str(root / "data"), SNAP_REAL_HOME=str(root / "home"),
                HOME=str(root / "home"), XDG_CONFIG_HOME=str(root / "home/.config"),
            )
            result = subprocess.run(
                ["sh", str(launcher), str(loader), "--inhibit-cache", str(root / "probe")],
                env=environment, capture_output=True, text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
