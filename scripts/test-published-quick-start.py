#!/usr/bin/env python3
"""Offline tests for check-published-quick-start.py."""

import importlib.util
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).with_name("check-published-quick-start.py")
SPEC = importlib.util.spec_from_file_location("published_quick_start", SCRIPT)
CHECKER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(CHECKER)


DEPENDENCIES = '''```toml
[dependencies]
qubit-fs = "0.3"
qubit-fs-local = "^0.11"
tempfile = "3"
```'''
PROGRAM = '''<!-- example: quick-start -->
```rust
fn main() { println!("report ready"); }
```'''


def readme(title="Quick Start", dependencies=DEPENDENCIES, program=PROGRAM):
    return f"# Project\n\n## {title}\n\n{dependencies}\n\n{program}\n\n## Next\n"


class PublishedQuickStartTests(unittest.TestCase):
    def test_extracts_english_and_chinese_sections(self):
        extracted = CHECKER.read_examples(readme(), readme("快速开始"))
        self.assertEqual(extracted.dependencies, "[dependencies]\nqubit-fs = \"0.3\"\nqubit-fs-local = \"^0.11\"\ntempfile = \"3\"")
        self.assertIn('println!("report ready")', extracted.rust)

    def test_rejects_missing_and_duplicate_blocks(self):
        with self.assertRaisesRegex(CHECKER.SmokeError, "dependencies"):
            CHECKER.read_examples(readme(dependencies=""), readme("快速开始", dependencies=""))
        with self.assertRaisesRegex(CHECKER.SmokeError, "exactly one"):
            CHECKER.read_examples(readme(dependencies=DEPENDENCIES + "\n" + DEPENDENCIES), readme("快速开始"))
        with self.assertRaisesRegex(CHECKER.SmokeError, "exactly one"):
            CHECKER.read_examples(readme(program=PROGRAM + "\n" + PROGRAM), readme("快速开始"))

    def test_rejects_bilingual_mismatch(self):
        changed = DEPENDENCIES.replace('"3"', '"4"')
        with self.assertRaisesRegex(CHECKER.SmokeError, "differ"):
            CHECKER.read_examples(readme(), readme("快速开始", dependencies=changed))

    def test_rejects_dependency_injection(self):
        forbidden = [
            '[dependencies]\nqubit-fs = { path = "../rs-fs" }',
            '[dependencies]\nqubit-fs = { git = "https://example.invalid/repo" }',
            '[patch.crates-io]\nqubit-fs = { path = "../rs-fs" }\n\n[dependencies]\nqubit-fs = "0.3"',
            '[replace]\n"qubit-fs:0.3.0" = { path = "../rs-fs" }\n\n[dependencies]\nqubit-fs = "0.3"',
            '[workspace]\nmembers = ["."]\n\n[dependencies]\nqubit-fs = "0.3"',
        ]
        for block in forbidden:
            with self.subTest(block=block), self.assertRaises(CHECKER.SmokeError):
                CHECKER.validate_dependencies(block)

    def test_cargo_failure_code_is_propagated_and_artifacts_preserved(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "README.md").write_text(readme(), encoding="utf-8")
            (root / "README.zh_CN.md").write_text(readme("快速开始"), encoding="utf-8")
            artifacts = root / "artifacts"
            calls = []

            observed = {}

            def fake_run(command, **kwargs):
                calls.append((command, kwargs))
                cwd = Path(kwargs["cwd"])
                observed["main_exists"] = (cwd / "src" / "main.rs").exists()
                (cwd / "Cargo.lock").write_text("# generated lock\n", encoding="utf-8")
                if "run" in command:
                    return CHECKER.subprocess.CompletedProcess(command, 17, "stdout", "cargo failed")
                return CHECKER.subprocess.CompletedProcess(command, 0, "tree", "")

            with mock.patch.object(CHECKER.subprocess, "run", side_effect=fake_run):
                code = CHECKER.run_smoke(root, artifacts)

            self.assertEqual(code, 17)
            self.assertTrue((artifacts / "Cargo.lock").exists())
            self.assertTrue((artifacts / "cargo-run.log").exists())
            self.assertIn("cargo failed", (artifacts / "cargo-run.log").read_text(encoding="utf-8"))
            self.assertEqual(len(calls), 2)
            self.assertTrue((artifacts / "cargo-tree.log").exists())
            self.assertNotEqual(calls[0][1]["cwd"], root)
            self.assertTrue(observed["main_exists"])
            self.assertIn("CARGO_TARGET_DIR", calls[0][1]["env"])
            self.assertTrue(calls[0][1]["env"]["CARGO_TARGET_DIR"].startswith(str(calls[0][1]["cwd"])))

    def test_success_runs_tree_and_requires_standalone_report_line(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "README.md").write_text(readme(), encoding="utf-8")
            (root / "README.zh_CN.md").write_text(readme("快速开始"), encoding="utf-8")
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                cwd = Path(kwargs["cwd"])
                (cwd / "Cargo.lock").write_text("# lock\n", encoding="utf-8")
                if "run" in command:
                    return CHECKER.subprocess.CompletedProcess(command, 0, "report ready\n", "")
                return CHECKER.subprocess.CompletedProcess(command, 0, "qubit-fs v0.3.0\n", "")

            with mock.patch.object(CHECKER.subprocess, "run", side_effect=fake_run):
                self.assertEqual(CHECKER.run_smoke(root, None), 0)
            self.assertEqual(len(calls), 2)

            with mock.patch.object(CHECKER.subprocess, "run", return_value=CHECKER.subprocess.CompletedProcess([], 0, "prefix report ready suffix\n", "")):
                self.assertEqual(CHECKER.run_smoke(root, None), 1)


if __name__ == "__main__":
    unittest.main()
