# Qubit FS

[![Rust CI](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs.svg?color=blue)](https://crates.io/crates/qubit-fs)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Qubit FS is a provider-neutral filesystem abstraction. Applications use the concrete
`FileSystem` and `AsyncFileSystem` facades; providers implement contracts only in
the `qubit_fs::spi` namespace. Provider discovery and configuration belong to
[`qubit-fs-registry`](https://crates.io/crates/qubit-fs-registry).

```toml
[dependencies]
qubit-fs = "0.2"
```

## Application API

Create a concrete facade from a provider SPI, then address logical resources with `Path`.

```rust,ignore
use qubit_fs::{FileSystem, Path, ReadOptions};
use qubit_fs::spi::FileSystemSpi;

fn read_metadata<S: FileSystemSpi + 'static>(provider: S) -> qubit_fs::FsResult<()> {
    let filesystem = FileSystem::from_spi(provider)?;
    let path = Path::parse("/reports/2026/summary.csv")?;
    let _metadata = filesystem.stat(&path)?;
    let _reader = filesystem.open_reader(&path, ReadOptions::default())?;
    Ok(())
}
```

Copy and rename report typed failures and preserve recovery state. Writers and temporary
handles retain explicit `abort`, `cleanup`, `keep`, or `persist` lifecycle operations after
recoverable failures. `AsyncFileSystem::begin_copy` returns an `AsyncCopyOperation`; poll its
execution future with the application's runtime and inspect its state after cancellation.

`Uri` rejects credential-bearing fields. `ConnectionUri` can carry credentials
for connection use, but masks them in `Display` and `Debug`; applications must
still avoid exposing the original value. `UserMetadata` keeps rejecting
credential-like keys even when an application installs allow rules in its
process-wide redaction default. `FileSystemProperties` is an immutable,
non-I/O snapshot with capabilities, limits, and logical-path constraints.

## Documentation

- [User guide](doc/user_guide.md)
- [用户指南](doc/user_guide.zh_CN.md)
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
