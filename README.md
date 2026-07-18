# Qubit FS

[![Rust CI](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs.svg?color=blue)](https://crates.io/crates/qubit-fs)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Qubit FS is a provider-neutral filesystem abstraction for local, remote, cloud,
distributed, and virtual storage backends.

The crate defines contracts rather than a concrete backend:

- `FileSystemProperties` exposes construction-time, non-I/O information;
- `FileSystem` provides synchronous operations;
- `AsyncFileSystem` provides runtime-neutral asynchronous operations;
- `FileSystemExt` and `AsyncFileSystemExt` provide bounded whole-resource
  helpers without expanding provider traits;
- file handles use `qubit_io::Input` / `Output` and
  `AsyncInput` / `AsyncOutput`;
- `FsUri` locates a resource while `FsPath` represents the provider-decoded
  path inside one configured filesystem;
- typed capabilities, requirements, outcomes, and errors preserve semantic
  differences between POSIX filesystems, object stores, cloud drives, and
  distributed filesystems;
- sync and async registries pass a complete `FileSystemConfig` to pluggable
  providers.

No local or remote provider is built into this crate. Applications assemble
backend crates at startup.

## Installation

```toml
[dependencies]
qubit-fs = "0.2"
```

## Synchronous Resolution

```rust
use qubit_fs::{
    CredentialRef,
    FileResource,
    FileSystemConfig,
    FileSystemRegistry,
    FsResult,
    FsUri,
};

fn resolve_report(
    registry: &FileSystemRegistry,
) -> FsResult<FileResource> {
    let uri = FsUri::parse(
        "s3://reports/2026/summary.csv?region=us-east-1",
    )?;
    let config = FileSystemConfig::new(uri)
        .with_credentials(CredentialRef::Profile("analytics".into()));
    registry.resource(&config)
}
```

The registry selects by URI scheme unless `FileSystemConfig` contains an
explicit provider selection. `resource_uri()` is the URI-only convenience
method.

## Asynchronous Resolution

```rust
use qubit_fs::{
    AsyncFileResource,
    AsyncFileSystemRegistry,
    FileSystemConfig,
    FsResult,
    FsUri,
};

async fn resolve_report(
    registry: &AsyncFileSystemRegistry,
) -> FsResult<AsyncFileResource> {
    let config = FileSystemConfig::new(
        FsUri::parse("s3://reports/2026/summary.csv")?,
    );
    registry.resource_async(&config).await
}
```

Async filesystem methods use `_async` names. Opening is itself asynchronous and
returns an already-initialized `AsyncFileReader` or `AsyncFileWriter`.

## Semantic Guarantees

`AtomicityRequirement::Required` is a contract: a provider must reject an
unsupported guarantee before side effects, never silently downgrade it.
Successful write, rename, copy, and temporary-persist operations report the
atomicity and concrete publication method actually achieved.

Writers and temporary handles retain their provider sessions after recoverable
failures. Temporary persistence additionally reports
`PersistFailureState::{NotPublished, PublishedSourceRetained, Indeterminate}`
so callers can distinguish retry, cleanup, and reconciliation paths.

`FsUri` preserves the raw encoded path, ordered duplicate query pairs, and the
difference between `scheme:/path` and `scheme:///path`. Providers own URI-path
decoding. Literal path characters that require escaping must already be percent
encoded. Passwords, tokens, and other credential-like values are rejected from
URIs. `NonSensitiveMetadata` rejects credential-like keys recursively from all
debug-visible extensible metadata, including config options, filesystem and
file metadata, and operation diagnostics. Validation covers string maps and
JSON objects nested in arrays, while its `Debug` output prints keys only.
Scalar values cannot be classified reliably, so use `CredentialRef` for every
secret.

## Documentation

- [User guide](doc/user_guide.md)
- [用户指南](doc/user_guide.zh_CN.md)
- [中文架构设计](doc/file_system_design.zh_CN.md)
- [API reference](https://docs.rs/qubit-fs)

## Development

```bash
cargo test
./align-ci.sh
RS_CI_SKIP_TOOLCHAIN_UPDATE=1 ./ci-check.sh
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).

Copyright (c) 2025 - 2026 Haixing Hu.
