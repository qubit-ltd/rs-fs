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
- `FileSystemExt` and `AsyncFileSystemExt` provide whole-resource helpers that
  require an explicit caller byte budget;
- directory stream helpers require an explicit maximum entry count before
  collecting an enumeration into memory;
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

`stat` is a required filesystem operation rather than an optional capability.
`FileSystemCapabilities` therefore contains only optional guarantees, while
`FileSystemProperties::limits()` returns the provider's stable
`FileSystemLimits` snapshot. Every limit uses `FileSystemLimit` to distinguish
`Unknown`, `NotApplicable`, `Unbounded`, and an inclusive `Maximum(n)`.
Providers remain responsible for enforcing their declared finite limits on
direct operations; bound resources and whole-resource helpers preflight limits
when the request size or canonical path is already known.

## Bounded Aggregation

Whole-resource and whole-enumeration helpers never choose an implicit memory
limit. The caller supplies a budget for each operation:

```rust,ignore
let bytes = resource.read_all(8 * 1024 * 1024)?;
let entries = resource
    .list(ListOptions::default())?
    .collect_entries(10_000)?;
```

An exact-size result succeeds. If a minimal probe confirms additional bytes or
entries, the helper returns `FsErrorKind::ResourceLimitExceeded`. Provider
storage capacity and account quota failures remain `FsErrorKind::QuotaExceeded`.

## Native Path Text Encoding

`FsPath`, `FsName`, and `RelativeFsPath` store canonical UTF-8 path text. This
is a lossless representation of a provider's native path string, not a claim
that every provider natively uses UTF-8 and never a lossy display conversion.
Ordinary Unicode is kept readable; a literal `%`, controls, and opaque bytes
use uppercase `%XX` escapes.

| Native value or bytes | Canonical path text |
| --- | --- |
| `report-中文.txt` | `report-中文.txt` |
| `100%` | `100%25` |
| `66 6F 80 6F` | `fo%80o` |
| `line<LF>break` | `line%0Abreak` |
| Windows lone surrogate `D800` | `%ED%A0%80` |

`NativePathCodec` converts only this string representation. It does not split
components, interpret roots or separators, normalize dot segments, decode URI
syntax, or perform I/O. Use `OsStrPathCodec` for local native `OsStr` values,
`Utf8PathCodec` for protocols that guarantee strict UTF-8 bytes, and
`EscapedBytePathCodec` for opaque byte-oriented protocols such as compatible
SFTP or NFS servers. All three are zero-sized, standard-library-only codecs.

Canonical spelling is required: raw `%`, malformed escapes, lowercase hex, and
over-escaped ordinary Unicode are rejected. This gives one textual identity for
each native path and makes `Eq`/`Hash` safe for registry and cache keys. URI
percent encoding remains a separate layer: a canonical native path fragment
`%25` becomes `%2525` when encoded as a URI path component.

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
