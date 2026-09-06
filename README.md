# Qubit FS

[![Rust CI](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs.svg?color=blue)](https://crates.io/crates/qubit-fs)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

`qubit-fs` 0.3.0 is a provider-neutral, synchronous and asynchronous filesystem
abstraction for Rust 1.94 or later. It supplies application-facing concrete
facades—`FileSystem` and `AsyncFileSystem`—instead of choosing a storage backend
or an async runtime for you.

The crate has no built-in backend. Providers implement extension contracts under
`qubit_fs::spi`; provider discovery, configuration, and credential handling are
the responsibility of `qubit-fs-registry`. This keeps application code on the
public facade while allowing providers to be selected outside the core crate.

```toml
[dependencies]
qubit-fs = "0.3"
```

Synchronous APIs are enabled by default. Enable the asynchronous facade
explicitly when it is needed:

```toml
qubit-fs = { version = "0.3", features = ["async"] }
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

## Start here

- [English user guide](doc/user_guide.md)
- [中文用户指南](doc/user_guide.zh_CN.md)
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
