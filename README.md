# Qubit FS

[![Rust CI](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs.svg?color=blue)](https://crates.io/crates/qubit-fs)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Qubit FS is a pluggable filesystem abstraction for Rust.

It defines provider-neutral contracts for local filesystems, WebDAV, FTP, OSS,
HDFS, and other storage backends. The root crate intentionally does not define a
closed `FsKind` enum; concrete backends are registered through `qubit-spi`.

## Provider model

- Third-party providers implement `ProviderDefinition<FileSystemSpec>`, so the
  provider carries both creation behavior and its own descriptor.
- `FileSystemRegistry` is runtime mutable. Its clones share registrations and
  default-selection updates.
- Downstream code can call `resolve(&ProviderSelection)` or `resolve()`
  to obtain a `ResolvingServiceProvider<FileSystemSpec>`, then create a
  filesystem with an independent `FileSystemConfig`.
- `fs(&FsUri)` is the domain convenience API: it selects by URI scheme, creates
  the filesystem, and maps selection and creation failures separately into
  `FsError` while preserving their source errors.

## Example

```rust
use qubit_fs::{FileSystemRegistry, FsResult, FsUri};

fn read_report(registry: &FileSystemRegistry) -> FsResult<Vec<u8>> {
    let uri = FsUri::parse("file:///var/data/report.csv")?;
    let resource = registry.resource(&uri)?;
    resource.read_all()
}

fn configure() -> FsResult<FileSystemRegistry> {
    let registry = FileSystemRegistry::default();
    // A backend crate can register its self-described provider at startup:
    // qubit_fs_local::register_provider(&registry)?;
    Ok(registry)
}
```

## Installation

```toml
[dependencies]
qubit-fs = "0.2"
```

## Documentation

- [User guide](doc/user_guide.md)
- [用户指南](doc/user_guide.zh_CN.md)
- [中文设计文档](doc/file_system_design.zh_CN.md)

## Testing

```bash
# Core API with the default empty feature set
cargo test --no-default-features

# Core API plus regex validation
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
