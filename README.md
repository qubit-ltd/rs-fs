# Qubit FS

Qubit FS is a pluggable filesystem abstraction for Rust.

It defines provider-neutral contracts for local filesystems, WebDAV, FTP, OSS,
HDFS, and other storage backends. The root crate intentionally does not define a
closed `FsKind` enum; concrete backends are registered through `qubit-spi`.

## Core concepts

- `FileSystem`: backend-neutral filesystem operations.
- `FsPath`: provider-local path value.
- `FsUri`: full URI used for provider selection.
- `FileResource`: resolved URI represented as a filesystem plus local path.
- `FileSystems`: process-wide singleton facade.
- `FileSystemRegistry`: explicit isolated registry for tests or embedded use.

## Example

```rust
use qubit_fs::{FileSystems, FsResult};

fn read_report() -> FsResult<Vec<u8>> {
    // Provider registration normally happens during application bootstrap:
    // FileSystems::register(LocalFileSystemProvider::new())?;

    let resource = FileSystems::resource("file:///var/data/report.csv")?;
    resource.read_all()
}
```

## Documentation

- [User guide](doc/user_guide.md)
- [用户指南](doc/user_guide.zh_CN.md)
- [中文设计文档](doc/file_system_design.zh_CN.md)
