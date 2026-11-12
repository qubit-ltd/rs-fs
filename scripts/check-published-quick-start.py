#!/usr/bin/env python3
"""Run the README Quick Start using published Cargo dependencies only."""

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path


class SmokeError(Exception):
    """An input README cannot safely define a standalone Cargo project."""


@dataclass(frozen=True)
class Example:
    dependencies: str
    rust: str


def section(text, heading):
    matches = list(re.finditer(r"(?m)^##\s+(.+?)\s*$", text))
    selected = [index for index, match in enumerate(matches) if match.group(1).strip() == heading]
    if len(selected) != 1:
        raise SmokeError(f"expected exactly one {heading!r} section, found {len(selected)}")
    index = selected[0]
    start = matches[index].end()
    end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
    return text[start:end]


def one_match(pattern, text, label):
    matches = list(re.finditer(pattern, text, re.DOTALL))
    if len(matches) != 1:
        raise SmokeError(f"expected exactly one {label} block, found {len(matches)}")
    return matches[0].group(1).strip()


def extract(text, heading):
    content = section(text, heading)
    dependencies = one_match(r"```toml\s*\n(.*?)\n```", content, "dependencies TOML")
    marker = "<!-- example: quick-start -->"
    if content.count(marker) != 1:
        raise SmokeError(f"expected exactly one {marker!r} marker, found {content.count(marker)}")
    rust = one_match(re.escape(marker) + r"\s*\n```rust\s*\n(.*?)\n```", content, "Quick Start Rust")
    return Example(dependencies, rust)


def validate_dependencies(block):
    try:
        data = tomllib.loads(block)
    except tomllib.TOMLDecodeError as error:
        raise SmokeError(f"invalid Quick Start TOML: {error}") from error
    banned = {"patch", "replace", "workspace"}.intersection(data)
    if banned:
        raise SmokeError(f"forbidden Cargo manifest section(s): {', '.join(sorted(banned))}")
    dependencies = data.get("dependencies")
    if not isinstance(dependencies, dict) or not dependencies:
        raise SmokeError("Quick Start TOML must define a non-empty [dependencies] table")
    if set(data) != {"dependencies"}:
        extra = ", ".join(sorted(set(data) - {"dependencies"}))
        raise SmokeError(f"only [dependencies] is allowed in Quick Start TOML; found {extra}")
    for name, requirement in dependencies.items():
        if isinstance(requirement, dict) and ({"path", "git"} & requirement.keys()):
            raise SmokeError(f"dependency {name!r} must not use path or git")
        if not isinstance(requirement, (str, dict)):
            raise SmokeError(f"dependency {name!r} has an unsupported specification")
    return data


def read_examples(english, chinese):
    left = extract(english, "Quick Start")
    right = extract(chinese, "快速开始")
    if left.dependencies != right.dependencies:
        raise SmokeError("English and Chinese Quick Start dependency blocks differ")
    if left.rust != right.rust:
        raise SmokeError("English and Chinese Quick Start Rust examples differ")
    validate_dependencies(left.dependencies)
    return left


def copy_artifacts(project, artifacts_dir):
    if artifacts_dir is None:
        return
    artifacts_dir.mkdir(parents=True, exist_ok=True)
    for name in ("Cargo.lock", "cargo-run.log", "cargo-tree.log"):
        source = project / name
        if source.is_file():
            shutil.copy2(source, artifacts_dir / name)


def run_smoke(repository, artifacts_dir=None):
    try:
        example = read_examples(
            (repository / "README.md").read_text(encoding="utf-8"),
            (repository / "README.zh_CN.md").read_text(encoding="utf-8"),
        )
    except (OSError, SmokeError) as error:
        print(f"published Quick Start: {error}", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory(prefix="published-quick-start-") as temporary:
        project = Path(temporary)
        (project / "src").mkdir()
        (project / "Cargo.toml").write_text(
            "[package]\nname = \"published-quick-start-smoke\"\nversion = \"0.0.0\"\nedition = \"2024\"\nrust-version = \"1.94\"\n\n"
            + example.dependencies + "\n",
            encoding="utf-8",
        )
        (project / "src" / "main.rs").write_text(example.rust + "\n", encoding="utf-8")
        environment = os.environ.copy()
        environment["CARGO_TARGET_DIR"] = str(project / "target")
        run_command = ["cargo", "run", "--manifest-path", str(project / "Cargo.toml")]
        tree_command = ["cargo", "tree", "--manifest-path", str(project / "Cargo.toml")]
        run_result = None
        tree_result = None
        return_code = 0
        try:
            run_result = subprocess.run(run_command, cwd=project, env=environment, text=True, capture_output=True, check=False)
            (project / "cargo-run.log").write_text(
                f"$ {' '.join(run_command)}\nexit code: {run_result.returncode}\n\nstdout:\n{run_result.stdout}\nstderr:\n{run_result.stderr}",
                encoding="utf-8",
            )
            if run_result.returncode:
                print(f"cargo run failed with exit code {run_result.returncode}", file=sys.stderr)
                if run_result.stdout:
                    print(run_result.stdout, file=sys.stderr, end="" if run_result.stdout.endswith("\n") else "\n")
                if run_result.stderr:
                    print(run_result.stderr, file=sys.stderr, end="" if run_result.stderr.endswith("\n") else "\n")
                return_code = run_result.returncode
            elif "report ready" not in run_result.stdout.splitlines():
                print("published Quick Start succeeded but did not print a standalone 'report ready' line", file=sys.stderr)
                return_code = 1

            tree_result = subprocess.run(tree_command, cwd=project, env=environment, text=True, capture_output=True, check=False)
            (project / "cargo-tree.log").write_text(
                f"$ {' '.join(tree_command)}\nexit code: {tree_result.returncode}\n\nstdout:\n{tree_result.stdout}\nstderr:\n{tree_result.stderr}",
                encoding="utf-8",
            )
            if tree_result.returncode:
                print(f"cargo tree failed with exit code {tree_result.returncode}", file=sys.stderr)
                if tree_result.stderr:
                    print(tree_result.stderr, file=sys.stderr, end="" if tree_result.stderr.endswith("\n") else "\n")
                if not return_code:
                    return_code = tree_result.returncode
            if not return_code:
                print("published Quick Start completed: report ready")
        except FileNotFoundError as error:
            print(f"published Quick Start could not start Cargo: {error}", file=sys.stderr)
            return_code = 127
        finally:
            copy_artifacts(project, artifacts_dir)
        return return_code


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts-dir", type=Path, help="copy Cargo.lock and diagnostics into this directory")
    args = parser.parse_args(argv)
    repository = Path(__file__).resolve().parent.parent
    return run_smoke(repository, args.artifacts_dir)


if __name__ == "__main__":
    raise SystemExit(main())
