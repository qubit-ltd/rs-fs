# Qubit FS

[![Rust CI](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs.svg?color=blue)](https://crates.io/crates/qubit-fs)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

`qubit-fs` 0.5.0 is a provider-neutral, synchronous and asynchronous filesystem
abstraction for Rust 1.94 or later. It supplies application-facing concrete
facades—`FileSystem` and `AsyncFileSystem`—instead of choosing a storage backend
or an async runtime for you.

The crate has no built-in backend. Providers implement extension contracts under
`qubit_fs::spi`; provider discovery, configuration, and credential handling are
the responsibility of `qubit-fs-registry`. This keeps application code on the
public facade while allowing providers to be selected outside the core crate.

```toml
[dependencies]
qubit-fs = "0.5"
```

Synchronous APIs are enabled by default. Enable the asynchronous facade
explicitly when it is needed:

```toml
qubit-fs = { version = "0.5", features = ["async"] }
```

## Try a local report workflow

A report job can keep its reads and writes on `FileSystem` while provider setup
chooses the storage authority. This executable demo creates an isolated local
root, writes a report, reads it with a 1 KiB limit, and prints `report ready`.
The temporary directory is removed when the demo ends.

```toml
[dependencies]
qubit-fs = "0.5"
qubit-fs-local = "0.7"
tempfile = "3"
```

<!-- example: quick-start -->
```rust
use std::time::Duration;

use qubit_fs::Path;
use qubit_fs::directory::ListScope;
use qubit_fs::read::ReadOptions;
use qubit_fs::write::WriteOptions;
use qubit_fs_local::LocalCopyResourceLimits;
use qubit_fs_local::LocalDeleteResourceLimits;
use qubit_fs_local::LocalFileSystems;
use qubit_fs_local::LocalListResourceLimits;
use qubit_fs_local::LocalResourcePolicy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let list_budget = LocalListResourceLimits::new(8, 64, 4 * 1024, 4, Duration::from_secs(5))?;
    let copy_budget = LocalCopyResourceLimits::new(8, 64, 1024 * 1024, 4, Duration::from_secs(5))?;
    let delete_budget = LocalDeleteResourceLimits::new(8, 64, 4 * 1024, Duration::from_secs(5));
    let policy = LocalResourcePolicy::bounded(list_budget, copy_budget, delete_budget);
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

## What the facade makes explicit

- `Path` is a logical name inside one configured filesystem. `Uri` is the
  secret-free canonical addressing form, while `ConnectionUri` is configuration
  ingress: it may accept credentials but redacts them in `Display` and `Debug`.
- Default URI parsing uses the fixed standard redaction policy; pass an explicit
  policy to `Uri::parse_with_policy` or `ConnectionUri::parse_with_policy` when
  application-specific query names must be protected.
- Copy, rename, writes, and temporary-resource publication (including `keep`)
  preserve typed
  recovery facts. Inspect the relevant failure state before retrying, cleaning
  up, or reconciling a visible target.
- `exists` returns `false` only when `stat` reports `NotFound`; permission,
  authentication, timeout, and I/O failures remain errors.
- `DirectoryStream` reads entries incrementally. Consume it in a bounded loop
  instead of assuming that a directory is a preloaded collection.

- Listing uses explicit `ListScope::Path` or flat `ListScope::Namespace`; prefix reads add a byte range only when `RangeRead` is Guaranteed.

## Start here

- [English user guide](doc/user_guide.md)
- [English provider guide](doc/provider_guide.md)
- [中文用户指南](doc/user_guide.zh_CN.md)
- [中文 provider 指南](doc/provider_guide.zh_CN.md)
- [Architecture design](doc/file_system_design.md)
- [中文架构设计](doc/file_system_design.zh_CN.md)
- [API reference](https://docs.rs/qubit-fs)

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
