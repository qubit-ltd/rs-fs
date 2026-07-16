# Qubit FS User Manual

## 1. What Qubit FS is

`qubit-fs` is a provider-neutral filesystem abstraction for Rust. It defines the common API surface for filesystems such as local filesystems, FTP, WebDAV, OSS, S3, HDFS, in-memory test filesystems, and private storage services.

The crate is intentionally only the core abstraction layer. It does not ship a concrete local, OSS, FTP, HDFS, or WebDAV implementation. Concrete implementations should live in separate crates, for example:

- `qubit-fs-local`
- `qubit-fs-webdav`
- `qubit-fs-oss`
- `qubit-fs-hdfs`
- `qubit-fs-memory`

This separation keeps `qubit-fs` extensible. Adding a new backend should not require modifying the root crate.

## 2. Installation

Add `qubit-fs` to `Cargo.toml`:

```toml
[dependencies]
qubit-fs = "0.2"
```

If you implement or register providers, you will usually also need `qubit-spi`:

```toml
[dependencies]
qubit-fs = "0.2"
qubit-spi = "0.8"
```

If your implementation stores rich metadata, use `qubit-metadata`:

```toml
[dependencies]
qubit-metadata = "0.6"
```

Optional feature:

```toml
[dependencies]
qubit-fs = { version = "0.2", features = ["registry-cache"] }
```

The current public API is synchronous and object-safe. Async providers can be built as separate crates or future extension traits, but the core `FileSystem` trait is intentionally not tied to a runtime.

## 3. Core concepts

### 3.1 `FsUri`

`FsUri` is the full resource locator. It contains:

- `scheme`: provider selector such as `file`, `oss`, `s3`, `webdav`, or `hdfs`
- `authority`: optional host, bucket, namespace, endpoint, or user hint
- `path`: provider-local `FsPath`
- `query`: non-sensitive options stored in `qubit_metadata::Metadata`

Example:

```rust
use qubit_fs::FsUri;

fn main() -> qubit_fs::FsResult<()> {
    let uri = FsUri::parse("oss://my-bucket/reports/2026/a.csv?region=cn-hangzhou")?;

    assert_eq!("oss", uri.scheme);
    assert_eq!("/reports/2026/a.csv", uri.path.as_str());

    let authority = uri.authority.expect("bucket should exist");
    assert_eq!("my-bucket", authority.host);

    Ok(())
}
```

Do not store passwords, access keys, secret keys, or tokens in URI strings. Use `CredentialRef` or provider-specific secure configuration.

### 3.2 `FsPath`

`FsPath` is the path inside one filesystem instance. It is not `std::path::Path`.

`FsPath` rules:

- uses UTF-8 strings
- uses `/` as the only separator
- can represent absolute and relative paths
- rejects empty paths
- rejects NUL bytes
- normalizes repeated `/` and `.`
- rejects paths that escape above their root with `..`

Example:

```rust
use qubit_fs::FsPath;

fn main() -> qubit_fs::FsResult<()> {
    let path = FsPath::parse("/a//b/./c.txt")?;
    assert_eq!("/a/b/c.txt", path.as_str());
    assert_eq!(Some("c.txt"), path.file_name());
    assert_eq!(Some("txt"), path.file_extension());

    let parent = path.parent().expect("parent should exist");
    assert_eq!("/a/b", parent.as_str());

    let joined = parent.join("d.txt")?;
    assert_eq!("/a/b/d.txt", joined.as_str());

    Ok(())
}
```

Use `std::path::Path` only inside local filesystem providers. It is not a portable cross-provider path model.

### 3.3 `FileSystem`

`FileSystem` is the central provider-neutral trait:

```rust
use qubit_fs::{
    CopyOptions, CopyOutcome, CreateDirOptions, DeleteOptions, DirectoryStream,
    FileMetadata, FileReader, FileSystem, FileSystemMetadata, FileWriter, FsPath,
    FsResult, ListOptions, ReadOptions, RenameOptions, WriteOptions,
};

pub trait FileSystem: std::fmt::Debug + Send + Sync {
    fn metadata(&self) -> FileSystemMetadata;
    fn path_metadata(&self, path: &FsPath) -> FsResult<FileMetadata>;
    fn exists(&self, path: &FsPath) -> FsResult<bool>;
    fn list(&self, path: &FsPath, options: &ListOptions) -> FsResult<Box<dyn DirectoryStream>>;
    fn open_reader(&self, path: &FsPath, options: &ReadOptions) -> FsResult<Box<dyn FileReader>>;
    fn open_writer(&self, path: &FsPath, options: &WriteOptions) -> FsResult<Box<dyn FileWriter>>;
    fn create_dir(&self, path: &FsPath, options: &CreateDirOptions) -> FsResult<()>;
    fn delete(&self, path: &FsPath, options: &DeleteOptions) -> FsResult<()>;
    fn rename(&self, from: &FsPath, to: &FsPath, options: &RenameOptions) -> FsResult<()>;
    fn copy(&self, from: &FsPath, to: &FsPath, options: &CopyOptions) -> FsResult<CopyOutcome>;
}
```

Implementations must not pretend all backends are POSIX filesystems. Unsupported operations should return `FsErrorKind::UnsupportedOperation` and capabilities should report the limitation.

### 3.4 Provider registry

`qubit-fs` uses `qubit-spi` for provider registration. It does not define a fixed enum like `FsKind`.

A provider declares an id and optional aliases. Example:

- provider id: `local`
- alias: `file`
- URI accepted by registry: `file:///tmp/a.txt`

This keeps the core crate open for third-party providers.

## 4. Opening filesystems and resources

A provider must be registered before a URI can be resolved. Assemble providers
through a builder during application startup, then share the immutable registry:

```rust
use qubit_fs::{FileSystemRegistry, FsResult};

fn configure_filesystems() -> FsResult<FileSystemRegistry> {
    let mut builder = FileSystemRegistry::builder();
    // Provider registration functions come from backend crates.
    // qubit_fs_local::register_provider(&mut builder)?;
    // qubit_fs_oss::register_provider(&mut builder)?;
    Ok(builder.build())
}
```

Parse the URI, then use `FileSystemRegistry::fs()` to select a filesystem:

```rust
use qubit_fs::{FileSystemRegistry, FsResult, FsUri};

fn open_filesystem(registry: &FileSystemRegistry) -> FsResult<()> {
    let uri = FsUri::parse("file:///var/data/report.csv")?;
    let fs = registry.fs(&uri)?;
    let caps = fs.capabilities();
    println!("directories supported: {}", caps.directories);
    Ok(())
}
```

Use `FileSystemRegistry::resource()` when you need both the filesystem and the
provider-local path. It also accepts a parsed `FsUri`:

```rust
use qubit_fs::{FileSystemRegistry, FsResult, FsUri};

fn resolve_and_check(registry: &FileSystemRegistry) -> FsResult<bool> {
    let uri = FsUri::parse("oss://bucket/reports/a.csv")?;
    let resource = registry.resource(&uri)?;
    resource.exists()
}
```

The same builder pattern can create isolated registries for tests, plugins, or
embedded runtimes:

```rust
use qubit_fs::{FileSystemRegistry, FsResult, FsUri};

fn isolated_registry() -> FsResult<()> {
    let mut builder = FileSystemRegistry::builder();
    // qubit_fs_memory::register_provider(&mut builder)?;
    let registry = builder.build();
    let uri = FsUri::parse("mem:///hello.txt")?;
    let resource = registry.resource(&uri)?;
    println!("{}", resource.path().as_str());
    Ok(())
}
```

## 5. Common operations

### 5.1 Read all bytes

Use `FileSystemExt::read_all()` for small or medium resources that can fit in memory:

```rust
use qubit_fs::{FileSystem, FileSystemExt, FsPath, FsResult};

fn read_config(fs: &dyn FileSystem) -> FsResult<Vec<u8>> {
    let path = FsPath::parse("/config/app.toml")?;
    fs.read_all(&path)
}
```

For large files, use `open_reader()` and stream data yourself:

```rust
use std::io::Read;
use qubit_fs::{FileSystem, FsPath, FsResult, ReadOptions};

fn stream_read(fs: &dyn FileSystem) -> FsResult<Vec<u8>> {
    let path = FsPath::parse("/large.bin")?;
    let mut reader = fs.open_reader(&path, &ReadOptions::default())?;

    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).map_err(|error| {
        qubit_fs::FsError::with_source(
            qubit_fs::FsErrorKind::Io,
            qubit_fs::FsOperation::OpenReader,
            "failed to read stream",
            error,
        ).with_path(path.clone())
    })?;

    Ok(buf)
}
```

### 5.2 Range read

Providers that support `range_read` should honor `ReadOptions.offset` and `ReadOptions.length`:

```rust
use qubit_fs::{FileSystem, FsPath, FsResult, ReadOptions};

fn open_range(fs: &dyn FileSystem) -> FsResult<()> {
    let path = FsPath::parse("/video.bin")?;
    let options = ReadOptions {
        offset: Some(1024),
        length: Some(4096),
        ..ReadOptions::default()
    };

    let _reader = fs.open_reader(&path, &options)?;
    Ok(())
}
```

If a backend cannot support range reads, it should either ignore the range only when documented as safe or return `UnsupportedOperation`. Prefer returning an explicit error when honoring the option is required by caller semantics.

### 5.3 Write all bytes

Use `FileSystemExt::write_all()` for simple writes:

```rust
use qubit_fs::{FileSystem, FileSystemExt, FsPath, FsResult};

fn write_report(fs: &dyn FileSystem) -> FsResult<()> {
    let path = FsPath::parse("/reports/today.txt")?;
    fs.write_all(&path, b"report content")?;
    Ok(())
}
```

For advanced writes, use `open_writer()`, write bytes, then call `commit()`:

```rust
use std::io::Write;
use qubit_fs::{FileSystem, FsPath, FsResult, WriteMode, WriteOptions};

fn create_new(fs: &dyn FileSystem) -> FsResult<()> {
    let path = FsPath::parse("/reports/new.txt")?;
    let options = WriteOptions {
        create_parent: true,
        mode: WriteMode::CreateNew,
        content_type: Some("text/plain".to_owned()),
        ..WriteOptions::default()
    };

    let mut writer = fs.open_writer(&path, &options)?;
    if let Err(error) = writer.write_all(b"hello") {
        let _ = writer.abort();
        return Err(qubit_fs::FsError::with_source(
            qubit_fs::FsErrorKind::Io,
            qubit_fs::FsOperation::OpenWriter,
            "failed to write data",
            error,
        ).with_path(path));
    }

    writer.commit()?;
    Ok(())
}
```

Always call either `commit()` or `abort()` for writers. Remote providers may allocate multipart uploads, temporary objects, or server sessions.

### 5.4 Conditional write

Use `WriteMode::ConditionalReplace` when replacing only a known version:

```rust
use qubit_fs::{FileSystem, FsPath, FsResult, WriteMode, WriteOptions};

fn replace_if_version_matches(fs: &dyn FileSystem, etag: String) -> FsResult<()> {
    let path = FsPath::parse("/state.json")?;
    let options = WriteOptions {
        mode: WriteMode::ConditionalReplace { etag },
        ..WriteOptions::default()
    };

    let writer = fs.open_writer(&path, &options)?;
    writer.commit()?;
    Ok(())
}
```

Providers should return `FsErrorKind::PreconditionFailed` when the condition does not match.

### 5.5 Metadata and existence

```rust
use qubit_fs::{FileSystem, FileKind, FsPath, FsResult};

fn inspect(fs: &dyn FileSystem) -> FsResult<()> {
    let path = FsPath::parse("/data/input.csv")?;

    if !fs.exists(&path)? {
        return Ok(());
    }

    let metadata = fs.path_metadata(&path)?;
    match metadata.kind {
        FileKind::File | FileKind::Object => println!("file-like resource"),
        FileKind::Directory | FileKind::Prefix => println!("container-like resource"),
        FileKind::Symlink => println!("symbolic link"),
        FileKind::Other(_) => println!("provider-specific resource"),
    }

    if let Some(len) = metadata.len {
        println!("size: {len}");
    }
    if let Some(etag) = metadata.etag {
        println!("etag: {etag}");
    }

    Ok(())
}
```

`exists()` should not hide authentication, permission, timeout, or network errors. It should return `Ok(false)` only when absence is confirmed.

### 5.6 Listing

`list()` returns a `DirectoryStream`, so large remote listings can page internally:

```rust
use qubit_fs::{DirectoryStreamExt, FileSystem, FsPath, FsResult, ListOptions};

fn list_once(fs: &dyn FileSystem) -> FsResult<()> {
    let path = FsPath::parse("/reports")?;
    let options = ListOptions {
        include_metadata: true,
        page_size: Some(100),
        ..ListOptions::default()
    };

    let entries = fs.list(&path, &options)?.collect_entries()?;
    for entry in entries {
        println!("{}", entry.path);
    }

    Ok(())
}
```

For very large directories, consume the stream incrementally:

```rust
use qubit_fs::{FileSystem, FsPath, FsResult, ListOptions};

fn list_streaming(fs: &dyn FileSystem) -> FsResult<()> {
    let path = FsPath::parse("/big-prefix")?;
    let mut stream = fs.list(&path, &ListOptions::default())?;

    while let Some(entry) = stream.next_entry()? {
        println!("{}", entry.path);
    }

    Ok(())
}
```

### 5.7 Create directory or collection

```rust
use qubit_fs::{CreateDirOptions, FileSystem, FsPath, FsResult};

fn create_reports_dir(fs: &dyn FileSystem) -> FsResult<()> {
    let path = FsPath::parse("/reports/2026")?;
    fs.create_dir(
        &path,
        &CreateDirOptions {
            recursive: true,
            exists_ok: true,
            ..CreateDirOptions::default()
        },
    )?;
    Ok(())
}
```

Object stores may model directories as prefixes or marker objects. Check `fs.capabilities().empty_directories` before relying on empty directory persistence.

### 5.8 Delete

```rust
use qubit_fs::{DeleteOptions, FileSystem, FsPath, FsResult};

fn delete_tree(fs: &dyn FileSystem) -> FsResult<()> {
    let path = FsPath::parse("/tmp/job-123")?;
    fs.delete(
        &path,
        &DeleteOptions {
            recursive: true,
            missing_ok: true,
            ..DeleteOptions::default()
        },
    )?;
    Ok(())
}
```

`missing_ok = true` applies only to confirmed absence. It must not swallow permission or authentication errors.

### 5.9 Rename or move

```rust
use qubit_fs::{AtomicityRequirement, FileSystem, FsPath, FsResult, RenameOptions};

fn atomic_publish(fs: &dyn FileSystem) -> FsResult<()> {
    let tmp = FsPath::parse("/out/report.tmp")?;
    let final_path = FsPath::parse("/out/report.csv")?;

    fs.rename(
        &tmp,
        &final_path,
        &RenameOptions {
            overwrite: true,
            atomic: AtomicityRequirement::Required,
        },
    )?;

    Ok(())
}
```

When `atomic = Required`, providers must fail with `UnsupportedOperation` if atomic rename cannot be guaranteed. Object stores usually cannot provide true atomic rename.

## 6. Copy model

`copy()` handles file-like, object-like, directory tree, prefix tree, or collection copy within one filesystem instance.

### 6.1 Copy one file or object

```rust
use qubit_fs::{CopyConflictPolicy, CopyOptions, FileSystem, FsPath, FsResult};

fn copy_file(fs: &dyn FileSystem) -> FsResult<()> {
    let from = FsPath::parse("/input/a.csv")?;
    let to = FsPath::parse("/archive/a.csv")?;

    let mut options = CopyOptions::file();
    options.conflict = CopyConflictPolicy::Overwrite;
    options.create_parent = true;

    let outcome = fs.copy(&from, &to, &options)?;
    println!("copied bytes: {}", outcome.stats.bytes);
    println!("method: {:?}", outcome.method);

    Ok(())
}
```

### 6.2 Copy a tree

```rust
use qubit_fs::{CopyOptions, FileSystem, FsPath, FsResult, MetadataPreservePolicy};

fn copy_tree(fs: &dyn FileSystem) -> FsResult<()> {
    let from = FsPath::parse("/dataset")?;
    let to = FsPath::parse("/backup/dataset")?;

    let mut options = CopyOptions::tree();
    options.create_parent = true;
    options.preserve_metadata = MetadataPreservePolicy::Portable;
    options.continue_on_error = false;

    let outcome = fs.copy(&from, &to, &options)?;
    println!("files: {}", outcome.stats.files);
    println!("directories: {}", outcome.stats.directories);
    println!("objects: {}", outcome.stats.objects);
    println!("prefixes: {}", outcome.stats.prefixes);

    Ok(())
}
```

### 6.3 Require server-side copy

```rust
use qubit_fs::{CopyOptions, FileSystem, FsPath, FsResult, ServerSidePreference};

fn server_side_copy_only(fs: &dyn FileSystem) -> FsResult<()> {
    let from = FsPath::parse("/a.bin")?;
    let to = FsPath::parse("/b.bin")?;

    let mut options = CopyOptions::file();
    options.server_side = ServerSidePreference::Require;

    fs.copy(&from, &to, &options)?;
    Ok(())
}
```

If server-side copy is not supported, the provider should return `FsErrorKind::UnsupportedOperation`.

### 6.4 Cross-filesystem copy

`FileSystem::copy()` copies within one filesystem instance. Cross-provider copy should be implemented at a higher layer by resolving both URIs as `FileResource` values and streaming from source to destination.

```rust
use std::io::{Read, Write};
use qubit_fs::{FileSystemRegistry, FsError, FsErrorKind, FsOperation, FsResult, FsUri, ReadOptions, WriteOptions};

fn copy_between(registry: &FileSystemRegistry, from_uri: &str, to_uri: &str) -> FsResult<()> {
    let from_uri = FsUri::parse(from_uri)?;
    let to_uri = FsUri::parse(to_uri)?;
    let from = registry.resource(&from_uri)?;
    let to = registry.resource(&to_uri)?;

    let mut reader = from.open_reader(&ReadOptions::default())?;
    let mut writer = to.open_writer(&WriteOptions::default())?;

    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer).map_err(|error| {
            FsError::with_source(FsErrorKind::Io, FsOperation::OpenReader, "read failed", error)
                .with_path(from.path().clone())
        })?;
        if n == 0 {
            break;
        }
        if let Err(error) = writer.write_all(&buffer[..n]) {
            let _ = writer.abort();
            return Err(FsError::with_source(
                FsErrorKind::Io,
                FsOperation::OpenWriter,
                "write failed",
                error,
            ).with_path(to.path().clone()));
        }
    }

    writer.commit()?;
    Ok(())
}
```

A production cross-filesystem copier should also support checksums, progress reporting, metadata preservation, cancellation, retry, and cleanup on failure.

## 7. Temporary resources

Temporary resources model ownership and cleanup responsibility. They are useful because they provide `Drop` best-effort cleanup in addition to explicit methods.

### 7.1 Create temporary files and directories

```rust
use std::sync::Arc;
use qubit_fs::{
    FileSystem,
    FileSystemExt,
    FsPath,
    FsResult,
    PersistOptions,
    TempDir,
    TempDirOptions,
    TempFile,
    TempFileOptions,
    TempResources,
};

fn temp_file_publish(fs: Arc<dyn FileSystem>) -> FsResult<()> {
    let temp: Box<dyn TempFile> =
        TempResources::create_file(fs.clone(), &TempFileOptions::default())?;

    let staging_path = temp.path().clone();
    fs.write_all(&staging_path, b"generated report\n")?;

    let target = FsPath::parse("/published/final.txt")?;
    temp.persist(&target, &PersistOptions::default())?;

    Ok(())
}

fn temp_dir_workspace(fs: Arc<dyn FileSystem>) -> FsResult<()> {
    let workspace: Box<dyn TempDir> =
        TempResources::create_dir(fs.clone(), &TempDirOptions::default())?;

    let part_file = workspace.path().join("part-0001.csv")?;
    fs.write_all(&part_file, b"id,value\n1,42\n")?;

    let target = FsPath::parse("/published/report-parts")?;
    workspace.persist(&target, &PersistOptions::default())?;
    Ok(())
}
```

`TempResources::create_file(fs, options)` works as follows:

- It first asks the current `FileSystem` instance for `fs.temp_resource_factory()`.
- It calls `create_file(fs, options)` on that factory, allowing the filesystem to return either a native `TempFile` or a core fallback.
- The default factory is `ManagedTempResourceFactory`: it generates a temporary path, reserves an empty file with `open_writer(..., CreateNew)`, and returns `ManagedTempFile`.
- Factory implementations may reuse `TempResourceFactory::make_temp_path()` for the common naming format, or use provider-specific naming rules.

`TempResources::create_dir(fs, options)` follows the same rule:

- It first asks the current `FileSystem` instance for `fs.temp_resource_factory()`.
- It calls `create_dir(fs, options)` on that factory, allowing the filesystem to return either a native `TempDir` or a core fallback.
- The default factory is `ManagedTempResourceFactory`: it generates a temporary path, creates the directory with `create_dir(..., recursive=true)`, and returns `ManagedTempDir`.
- Factory implementations may reuse `TempResourceFactory::make_temp_path()` for the common naming format, or use provider-specific naming rules.

`TempResources` also provides convenience helpers:

```rust
let file1 = TempResources::create_default_file(fs.clone())?;
let file2 = TempResources::create_file_with_prefix(fs.clone(), "upload-")?;

let dir1 = TempResources::create_default_dir(fs.clone())?;
let dir2 = TempResources::create_dir_with_prefix(fs.clone(), "job-")?;
```

`TempFile` and `TempDir` share the same usage pattern:

- Create a cleanup-owning handle with `TempResources::create_file()` or `TempResources::create_dir()`.
- Get the temporary path with `path()`, then write content or child files through normal `FileSystem` APIs.
- Call `persist()` to publish the resource to its final target.
- If the resource should not be published, call `cleanup()` explicitly or call `keep()` to disable automatic cleanup and transfer the temporary path to another component.

`ManagedTempFile` and `ManagedTempDir` are the fallback default implementations. They use the underlying filesystem to reserve temporary paths. If the handle is dropped before `cleanup()`, `persist()`, or `keep()`, it attempts best-effort cleanup.

### 7.2 Custom temporary file name

```rust
use std::sync::Arc;
use qubit_fs::{FileSystem, FsPath, FsResult, TempFileOptions, TempResources};

fn create_named_temp(fs: Arc<dyn FileSystem>) -> FsResult<()> {
    let parent = FsPath::parse("/tmp")?;
    let options = TempFileOptions {
        parent: Some(parent),
        prefix: "upload-".to_owned(),
        suffix: ".part".to_owned(),
    };

    let temp = TempResources::create_file(fs, &options)?;
    println!("temporary path: {}", temp.path());
    temp.cleanup()?;
    Ok(())
}
```

### 7.3 Keep a temporary resource

```rust
use std::sync::Arc;
use qubit_fs::{FileSystem, FsResult, TempDirOptions, TempResources};

fn keep_temp_dir(fs: Arc<dyn FileSystem>) -> FsResult<()> {
    let temp = TempResources::create_dir(fs, &TempDirOptions::default())?;
    let retained_path = temp.keep()?;
    println!("kept temp dir at {}", retained_path);
    Ok(())
}
```

`keep()` disables automatic cleanup and returns the temporary path. Use it when ownership is intentionally transferred to another component.

### 7.4 Persist with copy-delete fallback

```rust
use std::sync::Arc;
use qubit_fs::{AtomicityRequirement, FileSystem, FsPath, FsResult, PersistOptions, TempResources};

fn persist_with_fallback(fs: Arc<dyn FileSystem>) -> FsResult<()> {
    let temp = TempResources::create_file(fs, &Default::default())?;
    let target = FsPath::parse("/final/object.bin")?;

    let options = PersistOptions {
        overwrite: true,
        atomic: AtomicityRequirement::BestEffort,
        allow_copy_delete: true,
        ..PersistOptions::default()
    };

    temp.persist(&target, &options)?;
    Ok(())
}
```

If `rename()` returns `UnsupportedOperation` and `allow_copy_delete` is true, `ManagedTempFile` can fall back to `copy()` followed by `delete()`.

## 8. Errors

Most operations return `FsResult<T>`, which is an alias for `Result<T, FsError>`.

Important error kinds:

| Kind | Meaning |
| --- | --- |
| `NotFound` | The provider confirmed the resource does not exist. |
| `AlreadyExists` | Creation failed because the target exists. |
| `InvalidPath` | Path or URI is invalid. |
| `PermissionDenied` | The identity is known but lacks permission. |
| `AuthenticationFailed` | Credentials are missing or invalid. |
| `ProviderUnavailable` | No matching provider is registered or the provider cannot run. |
| `UnsupportedOperation` | The backend model does not support the operation. |
| `Conflict` | State conflict, such as deleting a non-empty directory without recursion. |
| `PreconditionFailed` | ETag or version condition failed. |
| `Timeout` | Provider operation timed out. |
| `QuotaExceeded` | Quota or capacity limit exceeded. |
| `DataCorruption` | Checksum or data integrity validation failed. |
| `Io` | Local or stream I/O error. |
| `Other` | Provider-specific fallback. |

Example:

```rust
use qubit_fs::{FileSystem, FsErrorKind, FsPath, FsResult};

fn delete_if_supported(fs: &dyn FileSystem) -> FsResult<()> {
    let path = FsPath::parse("/old")?;
    match fs.delete(&path, &Default::default()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == FsErrorKind::UnsupportedOperation => {
            println!("delete is not supported by this backend");
            Ok(())
        }
        Err(error) => Err(error),
    }
}
```

Provider implementations should preserve source errors with `FsError::with_source()` and attach path, target, and provider context when possible.

## 9. Metadata model

`FileMetadata` contains stable cross-provider fields and extension metadata.

Common fields include:

- `kind`: `File`, `Directory`, `Symlink`, `Object`, `Prefix`, or provider-specific `Other`
- `len`: optional byte length
- `modified_at`, `created_at`, `accessed_at`: optional timestamps
- `etag`: optional provider version or entity tag
- `content_type`: optional media type
- `checksum`: optional checksum
- `user_metadata`: user-defined metadata
- `provider_metadata`: backend-specific metadata

Example:

```rust
use qubit_fs::{FileMetadata, FileKind};

fn is_container(metadata: &FileMetadata) -> bool {
    metadata.is_directory_like()
        || matches!(metadata.kind, FileKind::Directory | FileKind::Prefix)
}
```

Use `provider_metadata` for fields that are meaningful only for one backend, such as HDFS replication, OSS storage class, FTP permission text, or WebDAV custom properties.

## 10. Capabilities

Always inspect capabilities before depending on non-basic behavior:

```rust
use qubit_fs::FileSystem;

fn explain_capabilities(fs: &dyn FileSystem) {
    let caps = fs.capabilities();

    println!("directories: {}", caps.directories);
    println!("range read: {}", caps.range_read);
    println!("append: {}", caps.append);
    println!("atomic rename: {}", caps.atomic_rename);
    println!("server-side copy: {}", caps.server_side_copy);
    println!("temporary files: {}", caps.temp_file);
}
```

Capabilities are not authorization. A provider may support recursive delete in general, while the current user still lacks permission for a specific path.

## 11. Implementing a new backend

This section shows how to build a new provider crate. The example is an in-memory filesystem because it is small enough to show the moving parts. Real providers follow the same shape.

### 11.1 Suggested crate layout

```text
rs-fs-memory/
  Cargo.toml
  src/
    lib.rs
    memory_file_system.rs
    memory_provider.rs
    memory_reader.rs
    memory_writer.rs
    memory_directory_stream.rs
    error_mapping.rs
  tests/
    memory_file_system_tests.rs
```

`Cargo.toml`:

```toml
[package]
name = "qubit-fs-memory"
version = "0.1.0"
edition = "2024"

[dependencies]
qubit-fs = "0.2"
qubit-spi = "0.8"
qubit-metadata = "0.6"
```

### 11.2 Define the filesystem type

```rust
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default, Clone)]
pub struct MemoryFileSystem {
    state: Arc<Mutex<MemoryState>>,
}

#[derive(Debug, Default)]
struct MemoryState {
    files: BTreeMap<String, Vec<u8>>,
}
```

A production backend would hold an SDK client, connection pool, root configuration, credential handle, and provider-specific options.

### 11.3 Implement metadata and capabilities

```rust
use qubit_fs::{
    FileMetadata, FileSystem, FileSystemCapabilities, FileSystemMetadata, FileKind,
    FsError, FsErrorKind, FsOperation, FsPath, FsResult,
};

impl FileSystem for MemoryFileSystem {
    fn metadata(&self) -> FileSystemMetadata {
        let mut metadata = FileSystemMetadata::new("memory");
        metadata.schemes.push("mem".to_owned());
        metadata.capabilities = FileSystemCapabilities {
            hierarchical_paths: true,
            directories: true,
            empty_directories: true,
            symlinks: false,
            range_read: true,
            append: false,
            random_write: false,
            atomic_rename: true,
            atomic_replace: true,
            conditional_write: false,
            server_side_copy: false,
            recursive_delete: true,
            temp_file: true,
            temp_dir: true,
            temp_persist: true,
            temp_persist_atomic: true,
            native_metadata: false,
        };
        metadata
    }

    fn path_metadata(&self, path: &FsPath) -> FsResult<FileMetadata> {
        let state = self.state.lock().expect("memory state should not be poisoned");
        let Some(bytes) = state.files.get(path.as_str()) else {
            return Err(FsError::new(FsErrorKind::NotFound, FsOperation::Metadata, "file not found")
                .with_path(path.clone())
                .with_provider("memory"));
        };

        let mut metadata = FileMetadata::new(FileKind::File);
        metadata.len = Some(bytes.len() as u64);
        Ok(metadata)
    }

    fn exists(&self, path: &FsPath) -> FsResult<bool> {
        let state = self.state.lock().expect("memory state should not be poisoned");
        Ok(state.files.contains_key(path.as_str()))
    }

    // Other trait methods are shown below.
}
```

Capability rules:

- report backend model capabilities, not current permissions
- return `UnsupportedOperation` when callers request unsupported behavior
- do not silently downgrade operations when the option says `Required`

### 11.4 Implement reading

Because `FileReader` has a blanket implementation for `Read + Send`, a `Cursor<Vec<u8>>` can be returned directly.

```rust
use std::io::Cursor;
use qubit_fs::{FileReader, ReadOptions};

impl MemoryFileSystem {
    fn read_bytes(&self, path: &FsPath, options: &ReadOptions) -> FsResult<Vec<u8>> {
        let state = self.state.lock().expect("memory state should not be poisoned");
        let Some(bytes) = state.files.get(path.as_str()) else {
            return Err(FsError::new(FsErrorKind::NotFound, FsOperation::OpenReader, "file not found")
                .with_path(path.clone())
                .with_provider("memory"));
        };

        let start = options.offset.unwrap_or(0) as usize;
        let end = match options.length {
            Some(length) => start.saturating_add(length as usize).min(bytes.len()),
            None => bytes.len(),
        };
        Ok(bytes.get(start..end).unwrap_or_default().to_vec())
    }
}

impl FileSystem for MemoryFileSystem {
    fn open_reader(&self, path: &FsPath, options: &ReadOptions) -> FsResult<Box<dyn FileReader>> {
        let bytes = self.read_bytes(path, options)?;
        Ok(Box::new(Cursor::new(bytes)))
    }

    // Other methods omitted in this excerpt.
}
```

For real remote providers, map HTTP, SDK, FTP, or HDFS errors into `FsErrorKind` and keep the original error as `source` when safe.

### 11.5 Implement writing

Writers must support `commit()` and `abort()`.

```rust
use std::io::{Result as IoResult, Write};
use qubit_fs::{FileWriter, WriteOutcome};

#[derive(Debug)]
struct MemoryWriter {
    fs: MemoryFileSystem,
    path: FsPath,
    buffer: Vec<u8>,
}

impl Write for MemoryWriter {
    fn write(&mut self, bytes: &[u8]) -> IoResult<usize> {
        self.buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl FileWriter for MemoryWriter {
    fn commit(self: Box<Self>) -> FsResult<WriteOutcome> {
        let mut state = self.fs.state.lock().expect("memory state should not be poisoned");
        let bytes_written = self.buffer.len() as u64;
        state.files.insert(self.path.as_str().to_owned(), self.buffer);

        Ok(WriteOutcome {
            bytes_written: Some(bytes_written),
            etag: None,
            diagnostics: qubit_metadata::Metadata::new(),
        })
    }

    fn abort(self: Box<Self>) -> FsResult<()> {
        Ok(())
    }
}
```

Then wire it into `open_writer()`:

```rust
use qubit_fs::{WriteMode, WriteOptions};

impl FileSystem for MemoryFileSystem {
    fn open_writer(&self, path: &FsPath, options: &WriteOptions) -> FsResult<Box<dyn FileWriter>> {
        if matches!(options.mode, WriteMode::Append) {
            return Err(FsError::new(
                FsErrorKind::UnsupportedOperation,
                FsOperation::OpenWriter,
                "append is not supported",
            ).with_path(path.clone()).with_provider("memory"));
        }

        Ok(Box::new(MemoryWriter {
            fs: self.clone(),
            path: path.clone(),
            buffer: Vec::new(),
        }))
    }

    // Other methods omitted in this excerpt.
}
```

Real object storage writers typically start a multipart upload, buffer parts, commit the upload in `commit()`, and abort the upload in `abort()` or `Drop`.

### 11.6 Implement listing

```rust
use qubit_fs::{DirEntry, DirectoryStream, ListOptions};

#[derive(Debug)]
struct MemoryDirectoryStream {
    entries: Vec<DirEntry>,
}

impl DirectoryStream for MemoryDirectoryStream {
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
        Ok(self.entries.pop())
    }
}

impl FileSystem for MemoryFileSystem {
    fn list(&self, path: &FsPath, _options: &ListOptions) -> FsResult<Box<dyn DirectoryStream>> {
        let state = self.state.lock().expect("memory state should not be poisoned");
        let prefix = if path.as_str() == "/" {
            "/".to_owned()
        } else {
            format!("{}/", path.as_str().trim_end_matches('/'))
        };

        let entries = state.files.keys()
            .filter(|key| key.starts_with(&prefix))
            .filter_map(|key| FsPath::parse(key).ok())
            .map(|path| DirEntry::new(path, FileKind::File))
            .collect();

        Ok(Box::new(MemoryDirectoryStream { entries }))
    }

    // Other methods omitted in this excerpt.
}
```

Remote providers should fetch pages lazily inside `next_entry()` instead of loading everything upfront when directories may be large.

### 11.7 Implement delete, rename, and copy

```rust
use qubit_fs::{CopyMethod, CopyOptions, CopyOutcome, CopyStats, DeleteOptions, RenameOptions};

impl FileSystem for MemoryFileSystem {
    fn delete(&self, path: &FsPath, options: &DeleteOptions) -> FsResult<()> {
        let mut state = self.state.lock().expect("memory state should not be poisoned");
        let existed = state.files.remove(path.as_str()).is_some();
        if !existed && !options.missing_ok {
            return Err(FsError::new(FsErrorKind::NotFound, FsOperation::Delete, "file not found")
                .with_path(path.clone())
                .with_provider("memory"));
        }
        Ok(())
    }

    fn rename(&self, from: &FsPath, to: &FsPath, options: &RenameOptions) -> FsResult<()> {
        let mut state = self.state.lock().expect("memory state should not be poisoned");
        if !options.overwrite && state.files.contains_key(to.as_str()) {
            return Err(FsError::new(FsErrorKind::AlreadyExists, FsOperation::Rename, "target exists")
                .with_path(from.clone())
                .with_target(to.clone())
                .with_provider("memory"));
        }
        let Some(bytes) = state.files.remove(from.as_str()) else {
            return Err(FsError::new(FsErrorKind::NotFound, FsOperation::Rename, "source not found")
                .with_path(from.clone())
                .with_target(to.clone())
                .with_provider("memory"));
        };
        state.files.insert(to.as_str().to_owned(), bytes);
        Ok(())
    }

    fn copy(&self, from: &FsPath, to: &FsPath, options: &CopyOptions) -> FsResult<CopyOutcome> {
        let mut state = self.state.lock().expect("memory state should not be poisoned");
        if !options.conflict.eq(&qubit_fs::CopyConflictPolicy::Overwrite)
            && state.files.contains_key(to.as_str())
        {
            return Err(FsError::new(FsErrorKind::AlreadyExists, FsOperation::Copy, "target exists")
                .with_path(from.clone())
                .with_target(to.clone())
                .with_provider("memory"));
        }
        let Some(bytes) = state.files.get(from.as_str()).cloned() else {
            return Err(FsError::new(FsErrorKind::NotFound, FsOperation::Copy, "source not found")
                .with_path(from.clone())
                .with_target(to.clone())
                .with_provider("memory"));
        };
        let len = bytes.len() as u64;
        state.files.insert(to.as_str().to_owned(), bytes);

        Ok(CopyOutcome::new(
            CopyStats { files: 1, bytes: len, ..CopyStats::default() },
            CopyMethod::Local,
        ))
    }

    // Other methods omitted in this excerpt.
}
```

For object stores, `rename()` is usually copy plus delete and should not claim atomic rename. For WebDAV, `MOVE` may be used. For HDFS, rename may be a strong commit primitive.

### 11.8 Implement provider registration

A provider creates filesystem instances from `FileSystemConfig`.

```rust
use std::sync::Arc;

use qubit_fs::{
    FileSystem,
    FileSystemConfig,
    FileSystemRegistryBuilder,
    FileSystemSpec,
    FsResult,
};
use qubit_spi::error::ProviderError;
use qubit_spi::{ProviderDescriptor, ProviderId, ServiceProvider};

#[derive(Debug, Default)]
pub struct MemoryFileSystemProvider;

impl ServiceProvider<FileSystemSpec> for MemoryFileSystemProvider {
    fn create(
        &self,
        _config: &FileSystemConfig,
    ) -> Result<Arc<dyn FileSystem>, ProviderError> {
        Ok(Arc::new(MemoryFileSystem::default()))
    }
}

pub fn register_provider(builder: &mut FileSystemRegistryBuilder) -> FsResult<()> {
    let descriptor = ProviderDescriptor::new(
        ProviderId::new("memory").expect("memory provider ID should be valid"),
    )
    .with_aliases(["mem"])
    .expect("memory provider aliases should be valid");
    builder.register(descriptor, MemoryFileSystemProvider)
}
```

Application usage:

```rust
use qubit_fs::{FileSystemRegistry, FsResult, FsUri};

fn main() -> FsResult<()> {
    let mut builder = FileSystemRegistry::builder();
    qubit_fs_memory::register_provider(&mut builder)?;
    let registry = builder.build();

    let uri = FsUri::parse("mem:///hello.txt")?;
    let resource = registry.resource(&uri)?;

    resource.write_all(b"hello")?;
    let bytes = resource.read_all()?;
    assert_eq!(b"hello", bytes.as_slice());

    Ok(())
}
```

### 11.9 Error mapping checklist for providers

When implementing a real backend, map native errors consistently:

| Native condition | Suggested `FsErrorKind` |
| --- | --- |
| 404, missing inode, missing object | `NotFound` |
| destination exists on create-new | `AlreadyExists` |
| invalid key, invalid path, invalid URI | `InvalidPath` |
| 401 or invalid credentials | `AuthenticationFailed` |
| 403 or ACL denial | `PermissionDenied` |
| 405 unsupported method, unsupported SDK operation | `UnsupportedOperation` |
| 409 conflict | `Conflict` |
| 412 ETag mismatch | `PreconditionFailed` |
| 423 locked resource | `Conflict` or provider-specific mapping |
| 429 or service throttle | `ProviderUnavailable` or `Timeout`, depending on semantics |
| checksum mismatch | `DataCorruption` |
| local stream error | `Io` |

Attach context:

```rust
use qubit_fs::{FsError, FsErrorKind, FsOperation, FsPath};

fn map_backend_error(path: &FsPath, source: std::io::Error) -> FsError {
    FsError::with_source(
        FsErrorKind::Io,
        FsOperation::OpenReader,
        "backend read failed",
        source,
    )
    .with_path(path.clone())
    .with_provider("my-provider")
}
```

### 11.10 Capability checklist for providers

Before publishing a provider, decide and test every capability:

| Capability | Questions to answer |
| --- | --- |
| `hierarchical_paths` | Are paths true directories or lexical keys? |
| `directories` | Can the backend represent directories or collections? |
| `empty_directories` | Can an empty directory survive without children? |
| `symlinks` | Can symlinks be read, copied, or created? |
| `range_read` | Can byte ranges be read efficiently? |
| `append` | Is append native and safe? |
| `random_write` | Can arbitrary offsets be modified? |
| `atomic_rename` | Is rename atomic for the relevant namespace? |
| `atomic_replace` | Can replacement avoid partially visible content? |
| `conditional_write` | Are ETag, generation, or version preconditions supported? |
| `server_side_copy` | Can the backend copy without streaming through the client? |
| `recursive_delete` | Can tree deletion be handled safely? |
| `temp_file` | Can temporary file reservation be implemented? |
| `temp_dir` | Can temporary directory reservation be implemented? |
| `temp_persist` | Can temp resources be persisted to final paths? |
| `temp_persist_atomic` | Can persistence be atomic? |
| `native_metadata` | Does the backend expose native metadata worth preserving? |

## 12. WebDAV provider guidance

A WebDAV provider should map WebDAV methods to `FileSystem` operations:

| `FileSystem` operation | WebDAV method |
| --- | --- |
| `metadata` | `PROPFIND Depth: 0` |
| `list` | `PROPFIND Depth: 1` or deeper paging strategy |
| `open_reader` | `GET` |
| `open_writer` | `PUT` staged until commit when possible |
| `create_dir` | `MKCOL` |
| `delete` | `DELETE` |
| `rename` | `MOVE` |
| `copy` | `COPY` |

Important WebDAV details:

- `ETag` should map to `FileMetadata.etag`.
- `Content-Type` should map to `FileMetadata.content_type`.
- WebDAV properties that have no stable cross-provider field should go to `provider_metadata`.
- `Depth` behavior must be documented because servers differ.
- `MOVE` and `COPY` may be best-effort and not atomic.
- HTTP 401, 403, 404, 405, 409, 412, 423, and 507 should be mapped carefully.

Provider skeleton:

```rust
#[derive(Debug)]
pub struct WebDavFileSystem {
    endpoint: String,
    client: reqwest::blocking::Client,
}

impl FileSystem for WebDavFileSystem {
    fn metadata(&self) -> FileSystemMetadata {
        let mut metadata = FileSystemMetadata::new("webdav");
        metadata.schemes.push("webdav".to_owned());
        metadata.schemes.push("webdavs".to_owned());
        metadata.capabilities.directories = true;
        metadata.capabilities.empty_directories = true;
        metadata.capabilities.server_side_copy = true;
        metadata
    }

    // Map PROPFIND, GET, PUT, MKCOL, DELETE, MOVE, and COPY here.
}
```

## 13. Local provider guidance

A local provider should keep `std::path::PathBuf` inside the provider and expose only `FsPath` through the core API.

Recommended local provider rules:

- configure a root directory
- map every `FsPath` under that root
- reject paths that escape the root
- handle symlink policy explicitly
- use atomic replace for `WriteMode::ReplaceAtomic`
- map `std::io::ErrorKind` to `FsErrorKind`
- use `qubit-local-files` for atomic writes, recursive copy, and local temporary resources where appropriate

Example mapping helper:

```rust
use std::path::{Path, PathBuf};
use qubit_fs::{FsError, FsErrorKind, FsOperation, FsPath, FsResult};

#[derive(Debug)]
pub struct LocalFileSystem {
    root: PathBuf,
}

impl LocalFileSystem {
    fn to_local_path(&self, path: &FsPath) -> FsResult<PathBuf> {
        let relative = path.as_str().trim_start_matches('/');
        let candidate = self.root.join(relative);

        if candidate.components().any(|component| matches!(component, std::path::Component::ParentDir)) {
            return Err(FsError::new(
                FsErrorKind::InvalidPath,
                FsOperation::ParsePath,
                "path escapes the local root",
            ).with_path(path.clone()).with_provider("local"));
        }

        Ok(candidate)
    }
}
```

## 14. Object storage provider guidance

Object stores such as OSS and S3 should not pretend to be full POSIX filesystems.

Recommended mapping:

| Concept | Mapping |
| --- | --- |
| `FsPath` | object key with normalized `/` separators |
| directory | prefix view or optional marker object |
| empty directory | usually unsupported unless marker objects are used |
| `metadata` | HEAD object or list result enrichment |
| `open_reader` | GET object, optionally range GET |
| `open_writer` | PUT object or multipart upload |
| `rename` | copy plus delete, not atomic |
| `copy` | server-side copy when possible |
| `etag` | provider ETag or generation/version id |
| user metadata | provider object metadata |

Key rules:

- `atomic_rename` should usually be false.
- `server_side_copy` should be true if object copy is supported.
- `WriteMode::ConditionalReplace` should map to generation or ETag preconditions if available.
- `TempResources` can work with object stores if `open_writer(CreateNew)` and `delete()` are implemented.
- Multipart upload must be aborted if writing fails.

## 15. Testing a provider

Provider test suites should cover at least:

1. URI parsing and provider registration.
2. Basic write, read, metadata, exists, list, delete.
3. Error mapping for not found, already exists, permission denied, authentication failure, and unsupported operation.
4. `WriteMode::CreateNew`, `CreateOrTruncate`, `ReplaceAtomic`, and provider-supported conditional modes.
5. `ReadOptions` range behavior if `range_read` is true.
6. `RenameOptions.atomic = Required` behavior.
7. `CopyOptions` conflict policies and server-side preference.
8. `TempResources::create_file()` and `create_dir()` if capabilities claim support.
9. `ManagedTempFile::persist()`, `cleanup()`, and `keep()` behavior.
10. Large listing behavior, including paging.
11. Metadata preservation and provider metadata fields.
12. Cleanup after failed writes or failed copy-delete fallback.

A minimal contract test can be written against `Arc<dyn FileSystem>` so the same tests can be reused by local, WebDAV, OSS, and in-memory providers.

## 16. Best practices

Use these rules when consuming or implementing `qubit-fs`:

| Topic | Practice |
| --- | --- |
| Path | Use `FsPath` in public filesystem APIs. Convert to backend-native paths internally. |
| URI | Use `FsUri` only for locating providers and initial configuration. Do not pass full URI strings into every operation. |
| Secrets | Do not put secrets in `FsUri.query`. Use `CredentialRef` or secure provider configuration. |
| Capabilities | Check capabilities before depending on advanced behavior. |
| Unsupported behavior | Return `UnsupportedOperation` rather than silently pretending support. |
| Writing | Always commit or abort writers. |
| Temporary resources | Use `cleanup()`, `persist()`, or `keep()` to make ownership explicit. |
| Copy | Use `CopyOutcome.method` and `CopyStats` for audit and diagnostics. |
| Errors | Preserve operation, path, target, provider, and source error where safe. |
| Provider crates | Keep concrete implementations outside `qubit-fs`. Register through `qubit-spi`. |

## 17. Current limitations

Current `qubit-fs` provides the core abstraction. It does not yet provide:

- a built-in local filesystem provider
- a built-in WebDAV provider
- built-in OSS/S3/HDFS providers
- async traits
- global automatic provider discovery
- cross-provider copy orchestration
- credential resolution implementation

These are expected to be implemented in separate crates or future extension layers.

## 18. Recommended adoption path

For application developers:

1. Add `qubit-fs` and one or more provider crates.
2. Create a `FileSystemRegistryBuilder` with `FileSystemRegistry::builder()`.
3. Register provider crates explicitly, then call `build()` once.
4. Parse URI strings as `FsUri` values and resolve them with `FileSystemRegistry::resource()`.
5. Use `FileResource` for resource-oriented operations or `FileSystem` with `FsPath` for lower-level code.
6. Check capabilities before relying on advanced behavior.
7. Treat errors by `FsErrorKind`, not provider-native error types.

For provider authors:

1. Implement `FileSystem` for your backend.
2. Implement `FileReader`, `FileWriter`, and `DirectoryStream` as needed.
3. Report accurate `FileSystemCapabilities`.
4. Map native errors to `FsErrorKind`.
5. Implement `ServiceProvider<FileSystemSpec>`.
6. Expose a `register_provider()` helper.
7. Add contract tests that run against `Arc<dyn FileSystem>`.
8. Document backend-specific semantics, especially atomicity, directory behavior, metadata, and credentials.
