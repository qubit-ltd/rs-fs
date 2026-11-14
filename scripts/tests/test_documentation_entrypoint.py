"""Regression tests for the documentation fixture shell entry point."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SOURCE_ROOT = Path(__file__).resolve().parents[2]
ENTRYPOINT = SOURCE_ROOT / "scripts/check-documentation-examples.sh"


class DocumentationEntrypointTests(unittest.TestCase):
    def run_entrypoint(self, *, python_exit=0, cargo_test_exit=0):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory) / "rs-fs"
            scripts = root / "scripts"
            scripts.mkdir(parents=True)
            entrypoint = scripts / "check-documentation-examples.sh"
            entrypoint.write_bytes(ENTRYPOINT.read_bytes())
            (root / "rs-fs-local/scripts").mkdir(parents=True)
            (root / "rs-local-files/scripts").mkdir(parents=True)
            (root / "rs-redact/scripts").mkdir(parents=True)

            bin_directory = Path(temporary_directory) / "bin"
            bin_directory.mkdir()
            log = Path(temporary_directory) / "calls.log"
            python_stub = bin_directory / "python3"
            python_stub.write_text(
                "#!/usr/bin/python3\n"
                "import os, pathlib, sys\n"
                "with pathlib.Path(os.environ['CALL_LOG']).open('a') as log:\n"
                "    log.write('python3 ' + ' '.join(sys.argv[1:]) + '\\n')\n"
                "raise SystemExit(int(os.environ['PYTHON_EXIT']))\n"
            )
            cargo_stub = bin_directory / "cargo"
            cargo_stub.write_text(
                "#!/usr/bin/python3\n"
                "import os, pathlib, sys\n"
                "with pathlib.Path(os.environ['CALL_LOG']).open('a') as log:\n"
                "    log.write('cargo ' + ' '.join(sys.argv[1:]) + '\\n')\n"
                "args = sys.argv[1:]\n"
                "raise SystemExit(int(os.environ['CARGO_TEST_EXIT']) if args[0] == 'test' else 0)\n"
            )
            python_stub.chmod(0o755)
            cargo_stub.chmod(0o755)

            environment = os.environ.copy()
            environment.update(
                PATH=f"{bin_directory}{os.pathsep}{environment['PATH']}",
                CALL_LOG=str(log),
                PYTHON_EXIT=str(python_exit),
                CARGO_TEST_EXIT=str(cargo_test_exit),
            )
            result = subprocess.run(
                ["bash", str(entrypoint)],
                cwd=root,
                env=environment,
                check=False,
                capture_output=True,
                text=True,
            )
            calls = log.read_text().splitlines() if log.exists() else []
            return result, calls

    def test_python_failure_stops_before_cargo(self):
        result, calls = self.run_entrypoint(python_exit=7)
        self.assertEqual(7, result.returncode)
        self.assertEqual(["python3 scripts/check-documentation.py"], calls)

    def test_cargo_test_failure_stops_before_cargo_run(self):
        result, calls = self.run_entrypoint(cargo_test_exit=9)
        self.assertEqual(9, result.returncode)
        self.assertEqual(
            [
                "python3 scripts/check-documentation.py",
                "cargo test --locked --manifest-path tests/fixtures/documentation_examples/Cargo.toml",
            ],
            calls,
        )

    def test_success_runs_checks_in_order_with_locked_cargo_commands(self):
        result, calls = self.run_entrypoint()
        self.assertEqual(0, result.returncode)
        self.assertEqual(
            [
                "python3 scripts/check-documentation.py",
                "cargo test --locked --manifest-path tests/fixtures/documentation_examples/Cargo.toml",
                "cargo run --locked --manifest-path tests/fixtures/documentation_examples/Cargo.toml --bin quick_start",
            ],
            calls,
        )


if __name__ == "__main__":
    unittest.main()
