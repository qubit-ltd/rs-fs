# Qubit FS

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
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

Licensed under Apache License 2.0. See [LICENSE](LICENSE).
