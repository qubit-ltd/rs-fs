# Qubit FS

[![Rust CI](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs.svg?color=blue)](https://crates.io/crates/qubit-fs)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

`qubit-fs` is a provider-neutral, synchronous and asynchronous filesystem
abstraction for Rust 1.94 or later. It supplies application-facing concrete
facades—`FileSystem` and `AsyncFileSystem`—instead of choosing a storage backend
or an async runtime for you.

## Installation

```toml
[dependencies]
qubit-fs = "0.7"
```

Synchronous APIs are enabled by default. Enable the asynchronous facade
explicitly when it is needed:

```toml
qubit-fs = { version = "0.7", features = ["async"] }
```

## Quick Start

A report job can keep its reads and writes on `FileSystem` while provider setup
chooses the storage authority. This executable demo creates an isolated local
root, writes a report, reads it with a 1 KiB limit, and prints `report ready`.
The temporary directory is removed when the demo ends.

```toml
[dependencies]
qubit-fs = "0.7"
qubit-fs-local = "0.8"
tempfile = "3"
```

<!-- example: quick-start -->
```rust
use qubit_fs::Path;
use qubit_fs::directory::ListScope;
use qubit_fs::read::ReadOptions;
use qubit_fs::write::WriteOptions;
use qubit_fs_local::LocalFileSystems;
use qubit_fs_local::LocalResourcePolicy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let policy = LocalResourcePolicy::standard();
    let filesystem = LocalFileSystems::rooted(directory.path(), policy)?;
    let path = Path::parse("/report.txt")?;
    filesystem.write_all(&path, b"report ready", WriteOptions::default())?;
    let bytes = filesystem.read_all(&path, ReadOptions::default(), 1024)?;
    assert_eq!(b"report ready", bytes.as_slice());
    let scope = ListScope::Path(Path::root());
    let mut entries = filesystem.list(&scope, Default::default())?;
    assert_eq!(entries.next_entry()?.expect("published report").path, path);
    assert!(entries.next_entry()?.is_none());
    println!("{}", String::from_utf8(bytes)?);
    Ok(())
}
```

For a report assembled in several writes, create a temporary file under the
same logical parent, write through its returned path, and call `persist` only
when generation is complete. The final report name then stays free of
incomplete output. If publication fails, use the retained publication fact,
source qualification, and `publication_target()` to decide between retry,
cleanup, and read-only reconciliation.

## Why This Project Exists

Application code often needs the same read, write, copy, and listing workflow
across local disks, object stores, and hosted filesystems, but provider SDKs
expose different path models, error shapes, and recovery behavior. Scattering
provider details through business logic makes retries, cleanup, and cancellation
hard to reason about after a partial failure.

`qubit-fs` keeps applications on concrete `FileSystem` and `AsyncFileSystem`
facades while providers implement extension contracts under `qubit_fs::spi`.
Provider discovery, configuration, and credential handling stay in
`qubit-fs-registry`, so the core crate does not embed a backend or async runtime.

## What It Provides—and What It Does Not

The stable application surface is the public facade API, typed paths and URIs,
explicit listing scopes, bounded reads, and recovery objects that retain
publication facts across failed writes, copies, and cancellations. Detailed
workflows, error tables, and operational limits are in the user guide; provider
integration is documented separately.

The facade makes these semantics explicit:

- `Path` is a logical name inside one configured filesystem. `Uri` is the
  secret-free canonical addressing form, while `ConnectionUri` is configuration
  ingress: it may accept credentials but redacts them in `Display` and `Debug`.
- Default URI parsing uses the fixed standard redaction policy; pass an explicit
  policy to `Uri::parse_with_policy` or `ConnectionUri::parse_with_policy` when
  application-specific query names must be protected.
- Copy, rename, writes, and temporary-resource publication (including `keep`)
  preserve typed recovery facts. Temporary persistence reports target
  publication separately from source qualification, and
  `publication_target()` retains an earlier confirmed target across rejected
  retries, cleanup failures, and asynchronous cancellation. Inspect both facts
  before retrying, cleaning up, or reconciling a visible target.
- `exists` returns `false` only when `stat` reports `NotFound`; permission,
  authentication, timeout, and I/O failures remain errors.
- `DirectoryStream` reads entries incrementally. Consume it in a bounded loop
  instead of assuming that a directory is a preloaded collection.
- Listing uses explicit `ListScope::Path` or flat `ListScope::Namespace`; prefix
  reads add a byte range only when `RangeRead` is Guaranteed.

The core crate has no built-in backend and does not select an async runtime.
It does not implement cross-filesystem move, guarantee every provider capability,
or turn object keys into hierarchical paths without provider-specific rules.
Platform roots and authority boundaries come from the configured provider.

## Learn More

- [English user guide](doc/user_guide.md)
- [中文用户手册](doc/user_guide.zh_CN.md)
- [English provider guide](doc/provider_guide.md)
- [中文 provider 指南](doc/provider_guide.zh_CN.md)
- [Architecture design](doc/file_system_design.md)
- [中文架构设计](doc/file_system_design.zh_CN.md)
- [API documentation on docs.rs](https://docs.rs/qubit-fs)
- [中文 README](README.zh_CN.md)
- [Repository](https://github.com/qubit-ltd/rs-fs)

## Testing

```bash
# Run tests with the default feature set
cargo test

# Run tests with all declared features
cargo test --all-features

# Project CI checks
./ci-check.sh

# Check code coverage
./coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public
API documentation and tests current, and run `./align-ci.sh` to format code and
`./ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-fs](https://github.com/qubit-ltd/rs-fs)
