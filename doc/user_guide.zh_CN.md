# Qubit FS 用户指南

## 1. Qubit FS 是什么

`qubit-fs` 是 Rust 的抽象文件系统核心层。它定义了一组 provider-neutral 的统一接口，用于描述本地文件系统、FTP、WebDAV、OSS、S3、HDFS、内存测试文件系统以及企业私有存储服务。

当前 crate 只提供核心抽象，不内置任何具体后端实现。具体实现应放在独立 crate 中，例如：

- `qubit-fs-local`
- `qubit-fs-webdav`
- `qubit-fs-oss`
- `qubit-fs-hdfs`
- `qubit-fs-memory`

这样设计的核心原因是可扩展性。新增一种文件系统后端时，不应该修改 `qubit-fs` 根 crate，也不应该让下游因为新增 `FsKind` 枚举分支而被迫升级。

## 2. 安装

在 `Cargo.toml` 中加入：

```toml
[dependencies]
qubit-fs = "0.2"
```

如果你要实现或注册 provider，通常还需要 `qubit-spi`：

```toml
[dependencies]
qubit-fs = "0.2"
qubit-spi = "0.8"
```

如果 provider 需要读写扩展元数据，建议使用 `qubit-metadata`：

```toml
[dependencies]
qubit-metadata = "0.6"
```

可选 feature：

```toml
[dependencies]
qubit-fs = { version = "0.2", features = ["registry-cache"] }
```

当前公开 API 是同步、对象安全的接口。异步 provider 可以通过独立 crate 或后续扩展 trait 实现，核心 `FileSystem` trait 不绑定 tokio 或其他运行时。

## 3. 核心概念

### 3.1 `FsUri`

`FsUri` 表示完整资源定位信息。它包含：

| 字段 | 含义 |
| --- | --- |
| `scheme` | provider selector，例如 `file`、`oss`、`s3`、`webdav`、`hdfs` |
| `authority` | 可选的 host、bucket、namespace、endpoint 或 username hint |
| `path` | provider-local 的 `FsPath` |
| `query` | 非敏感选项，使用 `qubit_metadata::Metadata` 保存 |

示例：

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

不要把 password、access key、secret key、token 放进 URI。凭据应通过 `CredentialRef` 或 provider 自己的安全配置传入。

### 3.2 `FsPath`

`FsPath` 表示某个 `FileSystem` 实例内部的路径。它不是 `std::path::Path`。

`FsPath` 的规则：

| 规则 | 说明 |
| --- | --- |
| 字符串 | 使用 UTF-8 字符串 |
| 分隔符 | 统一使用 `/` |
| 绝对/相对 | 可区分 absolute 和 relative |
| 空路径 | 拒绝空 path |
| NUL 字节 | 拒绝包含 NUL 字节的 path |
| 规范化 | 规范化重复 `/` 和 `.` |
| `..` | 拒绝向上逃逸 root 的路径 |

示例：

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

`std::path::Path` 只适合本地文件系统 provider 内部使用，不适合作为跨后端统一路径模型。原因包括 Windows 盘符、UNC、平台分隔符、Unix 非 UTF-8 路径、对象存储 key 语义等差异。

### 3.3 `FileSystem`

`FileSystem` 是核心 trait：

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

实现者不要把所有后端都伪装成 POSIX 文件系统。不支持的操作应返回 `FsErrorKind::UnsupportedOperation`，并在 `capabilities()` 中声明能力缺失。

### 3.4 Provider registry

`qubit-fs` 复用 `qubit-spi` 做 provider 注册，不定义固定 `FsKind` 枚举。

一个 provider 可以声明一个稳定 id 和多个 alias。例如：

| provider id | alias | 可打开 URI |
| --- | --- | --- |
| `local` | `file` | `file:///tmp/a.txt` |
| `memory` | `mem` | `mem:///a.txt` |
| `webdav` | `webdavs` | `webdavs://host/dav/a.txt` |

这种方式允许第三方后端自然接入，不需要修改核心 crate。

## 4. 打开文件系统和资源

使用前必须先注册 provider。应用可以在启动阶段创建 registry，注册第三方 provider，
然后把 registry 的 clone 共享给下游库。所有 clone 都能看到后续的运行时注册和默认选择更新：

```rust
use qubit_fs::{FileSystemRegistry, FsResult};

fn configure_filesystems() -> FsResult<FileSystemRegistry> {
    let registry = FileSystemRegistry::default();
    // provider 注册函数由后端 crate 提供。
    // qubit_fs_local::register_provider(&registry)?;
    // qubit_fs_oss::register_provider(&registry)?;
    Ok(registry)
}
```

先解析 URI，再用 `FileSystemRegistry::fs()` 选择文件系统实例：

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

如果需要同时得到文件系统实例和 provider-local path，使用
`FileSystemRegistry::resource()`；它同样接收解析后的 `FsUri`：

```rust
use qubit_fs::{FileSystemRegistry, FsResult, FsUri};

fn resolve_and_check(registry: &FileSystemRegistry) -> FsResult<bool> {
    let uri = FsUri::parse("oss://bucket/reports/a.csv")?;
    let resource = registry.resource(&uri)?;
    resource.exists()
}
```

测试、插件运行时或嵌入式场景也可以用空 registry 构造隔离 catalog：

```rust
use qubit_fs::{FileSystemRegistry, FsResult, FsUri};

fn isolated_registry() -> FsResult<()> {
    let registry = FileSystemRegistry::default();
    // qubit_fs_memory::register_provider(&registry)?;
    let uri = FsUri::parse("mem:///hello.txt")?;
    let resource = registry.resource(&uri)?;
    println!("{}", resource.path().as_str());
    Ok(())
}
```

## 5. 常用操作

### 5.1 读取完整内容

小文件或中等大小资源可以用 `FileSystemExt::read_all()`：

```rust
use qubit_fs::{FileSystem, FileSystemExt, FsPath, FsResult};

fn read_config(fs: &dyn FileSystem) -> FsResult<Vec<u8>> {
    let path = FsPath::parse("/config/app.toml")?;
    fs.read_all(&path)
}
```

大文件建议用 `open_reader()` 流式读取：

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

### 5.2 范围读取

支持 `range_read` 的 provider 应处理 `ReadOptions.offset` 和 `ReadOptions.length`：

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

如果后端无法支持范围读取，应返回 `UnsupportedOperation`，或者在文档中明确说明何时会安全地忽略 range option。

### 5.3 写入完整内容

简单写入使用 `FileSystemExt::write_all()`：

```rust
use qubit_fs::{FileSystem, FileSystemExt, FsPath, FsResult};

fn write_report(fs: &dyn FileSystem) -> FsResult<()> {
    let path = FsPath::parse("/reports/today.txt")?;
    fs.write_all(&path, b"report content")?;
    Ok(())
}
```

高级写入使用 `open_writer()`，写入后必须调用 `commit()`：

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

写入句柄必须以 `commit()` 或 `abort()` 结束。远端 provider 可能持有 multipart upload、临时对象或服务端 session。

### 5.4 条件写入

当你只想替换某个确定版本时，使用 `WriteMode::ConditionalReplace`：

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

条件不匹配时，provider 应返回 `FsErrorKind::PreconditionFailed`。

### 5.5 Metadata 和 exists

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

`exists()` 不应吞掉认证失败、权限不足、超时或网络错误。只有后端明确确认资源不存在时，才返回 `Ok(false)`。

### 5.6 列举目录、prefix 或 collection

`list()` 返回 `DirectoryStream`，因此远端后端可以在内部分页：

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

大目录应流式消费：

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

### 5.7 创建目录或 collection

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

对象存储通常只支持 prefix 视图，未必能真实保留空目录。依赖空目录语义前，应检查 `fs.capabilities().empty_directories`。

### 5.8 删除

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

`missing_ok = true` 只表示“确认不存在也算成功”，不表示吞掉认证、权限、网络错误。

### 5.9 重命名或移动

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

当 `atomic = Required` 时，如果后端不能保证原子 rename，必须返回 `UnsupportedOperation`，不能偷偷降级。对象存储通常无法提供真正的原子 rename。

## 6. 复制模型

`copy()` 用于同一个 `FileSystem` 实例内部复制文件、对象、目录树、prefix tree 或 WebDAV collection。

### 6.1 复制单个文件或对象

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

### 6.2 复制目录树或 prefix tree

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

### 6.3 强制 server-side copy

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

如果后端不支持 server-side copy，应返回 `FsErrorKind::UnsupportedOperation`。

### 6.4 跨文件系统复制

`FileSystem::copy()` 只处理同一个文件系统实例内部复制。跨 provider 复制应由更上层把两个 URI 解析成 `FileResource`，然后从源流式读、向目标流式写。

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

生产级跨文件系统复制还应处理 checksum、进度、metadata 保留、取消、重试和失败清理。

## 7. 临时资源

临时资源表示“拥有清理责任的句柄”。它们有意义的关键点是：除了显式 `cleanup()`、`persist()`、`keep()`，还提供 `Drop` 尽力清理。

### 7.1 创建临时文件和临时目录

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

`TempResources::create_file(fs, options)` 的实际行为是：

- 先从当前 `FileSystem` 实例取得 `fs.temp_resource_factory()`。
- 调用该 factory 的 `create_file(fs, options)`，由当前文件系统决定返回 native `TempFile`，还是使用核心层 fallback。
- 默认 factory 是 `ManagedTempResourceFactory`：它会生成临时路径，使用 `open_writer(..., CreateNew)` 预留空文件，然后返回 `ManagedTempFile`。
- factory 实现可以复用 `TempResourceFactory::make_temp_path()` 生成统一格式的临时路径，也可以按后端需要使用自己的命名格式。

`TempResources::create_dir(fs, options)` 同理：

- 先从当前 `FileSystem` 实例取得 `fs.temp_resource_factory()`。
- 调用该 factory 的 `create_dir(fs, options)`，由当前文件系统决定返回 native `TempDir`，还是使用核心层 fallback。
- 默认 factory 是 `ManagedTempResourceFactory`：它会生成临时路径，调用 `create_dir(..., recursive=true)` 创建目录，然后返回 `ManagedTempDir`。
- factory 实现可以复用 `TempResourceFactory::make_temp_path()` 生成统一格式的临时路径，也可以按后端需要使用自己的命名格式。

`TempResources` 还提供常用便捷入口：

```rust
let file1 = TempResources::create_default_file(fs.clone())?;
let file2 = TempResources::create_file_with_prefix(fs.clone(), "upload-")?;

let dir1 = TempResources::create_default_dir(fs.clone())?;
let dir2 = TempResources::create_dir_with_prefix(fs.clone(), "job-")?;
```

`TempFile` 和 `TempDir` 的共同使用方式是：

- 通过 `TempResources::create_file()` 或 `TempResources::create_dir()` 创建拥有清理责任的句柄。
- 通过 `path()` 得到临时路径，再用普通 `FileSystem` API 写入内容或子文件。
- 成功后调用 `persist()` 发布到目标路径。
- 如果不想发布，调用 `cleanup()` 显式清理，或调用 `keep()` 放弃自动清理并把临时路径交给外部组件。

`ManagedTempFile` 和 `ManagedTempDir` 是 fallback 默认实现，会通过底层文件系统预留临时路径。如果句柄在 `cleanup()`、`persist()` 或 `keep()` 之前被 drop，会尝试尽力清理。

### 7.2 自定义临时文件路径模式

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

### 7.3 保留临时目录

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

`keep()` 会解除自动清理责任，并返回临时资源路径。适用于有意把所有权转移给其他组件的场景。

### 7.4 使用 copy-delete 作为 persist 降级方案

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

如果 `rename()` 返回 `UnsupportedOperation`，且 `allow_copy_delete = true`，`ManagedTempFile` 可以降级为 `copy()` 后 `delete()`。

## 8. 错误处理

大部分操作返回 `FsResult<T>`，即 `Result<T, FsError>`。

常见错误类型：

| Kind | 含义 |
| --- | --- |
| `NotFound` | 后端确认资源不存在 |
| `AlreadyExists` | 创建失败，因为目标已存在 |
| `InvalidPath` | 路径或 URI 非法 |
| `PermissionDenied` | 身份有效，但没有权限 |
| `AuthenticationFailed` | 凭据缺失或无效 |
| `ProviderUnavailable` | 未注册 provider，或 provider 当前不可用 |
| `UnsupportedOperation` | 后端模型不支持该操作 |
| `Conflict` | 状态冲突，例如非递归删除非空目录 |
| `PreconditionFailed` | ETag 或版本条件不满足 |
| `Timeout` | 操作超时 |
| `QuotaExceeded` | 配额或容量不足 |
| `DataCorruption` | checksum 或完整性验证失败 |
| `Io` | 本地或流式 I/O 错误 |
| `Other` | provider-specific fallback |

示例：

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

provider 实现应尽量用 `FsError::with_source()` 保留底层错误，并附加 path、target、provider 等上下文。

## 9. Metadata 模型

`FileMetadata` 包含跨后端稳定字段，也包含扩展 metadata。

常见字段：

| 字段 | 含义 |
| --- | --- |
| `kind` | `File`、`Directory`、`Symlink`、`Object`、`Prefix` 或 `Other` |
| `len` | 可选内容长度 |
| `modified_at`、`created_at`、`accessed_at` | 可选时间戳 |
| `etag` | 可选 provider version 或 entity tag |
| `content_type` | 可选 media type |
| `checksum` | 可选 checksum |
| `user_metadata` | 用户自定义 metadata |
| `provider_metadata` | 后端专属 metadata |

示例：

```rust
use qubit_fs::{FileMetadata, FileKind};

fn is_container(metadata: &FileMetadata) -> bool {
    metadata.is_directory_like()
        || matches!(metadata.kind, FileKind::Directory | FileKind::Prefix)
}
```

HDFS replication、OSS storage class、FTP 权限文本、WebDAV 自定义属性等字段应放到 `provider_metadata`，不要强行塞进跨后端稳定字段。

## 10. 能力模型

使用高级行为前，应先查看 capabilities：

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

capabilities 描述后端模型能力，不表示当前用户一定有权限。例如 provider 支持 recursive delete，但当前账号仍可能对某个路径返回 `PermissionDenied`。

## 11. 如何扩展实现一个新后端

本节用内存文件系统说明如何实现 provider。真实 Local、WebDAV、OSS、HDFS provider 的结构类似。

### 11.1 推荐 crate 结构

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

`Cargo.toml` 示例：

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

### 11.2 定义文件系统类型

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

生产级 provider 通常会持有 SDK client、连接池、root 配置、credential handle 和 provider-specific options。

### 11.3 实现 metadata 和 capabilities

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

    // 其他 trait 方法见下文。
}
```

能力声明原则：

| 原则 | 说明 |
| --- | --- |
| 描述模型能力 | capabilities 不是权限检查 |
| 不支持就报错 | 不要假装支持高级语义 |
| Required 不能降级 | 调用方要求原子或 server-side 时，不支持必须失败 |

### 11.4 实现读取

`FileReader` 对 `Read + Send` 有 blanket impl，所以 `Cursor<Vec<u8>>` 可以直接作为 reader 返回。

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

    // 其他方法略。
}
```

真实远端 provider 应把 HTTP、SDK、FTP、HDFS 错误映射到 `FsErrorKind`，必要时把原始错误放入 source。

### 11.5 实现写入

writer 必须支持 `commit()` 和 `abort()`。

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

接入 `open_writer()`：

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

    // 其他方法略。
}
```

对象存储 writer 通常会在内部创建 multipart upload，`commit()` 完成 upload，`abort()` 中止 upload。

### 11.6 实现 list

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

    // 其他方法略。
}
```

远端 provider 不应一次性加载巨大目录，应该在 `next_entry()` 中按页拉取。

### 11.7 实现 delete、rename、copy

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

    // 其他方法略。
}
```

对象存储的 `rename()` 通常是 copy + delete，不能声明 atomic rename。WebDAV 可映射到 `MOVE`。HDFS 的 rename 往往可作为强 commit 原语。

### 11.8 实现 provider 注册

provider 根据 `FileSystemConfig` 创建文件系统实例。

```rust
use std::sync::Arc;

use qubit_fs::{
    FileSystem,
    FileSystemConfig,
    FileSystemRegistry,
    FileSystemSpec,
    FsResult,
};
use qubit_spi::error::ProviderCreationError;
use qubit_spi::{
    ProviderDefinition,
    ProviderDescriptor,
    ProviderId,
    ServiceProvider,
};

#[derive(Debug, Default)]
pub struct MemoryFileSystemProvider;

impl ServiceProvider<FileSystemSpec> for MemoryFileSystemProvider {
    fn create(
        &self,
        _config: &FileSystemConfig,
    ) -> Result<Arc<dyn FileSystem>, ProviderCreationError> {
        Ok(Arc::new(MemoryFileSystem::default()))
    }
}

impl ProviderDefinition<FileSystemSpec> for MemoryFileSystemProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            ProviderId::new("memory")
                .expect("memory provider ID should be valid"),
        )
        .with_aliases(["mem"])
        .expect("memory provider aliases should be valid")
    }
}

pub fn register_provider(registry: &FileSystemRegistry) -> FsResult<()> {
    registry.register(MemoryFileSystemProvider)
}
```

应用侧使用：

```rust
use qubit_fs::{FileSystemRegistry, FsResult, FsUri};

fn main() -> FsResult<()> {
    let registry = FileSystemRegistry::default();
    qubit_fs_memory::register_provider(&registry)?;

    let uri = FsUri::parse("mem:///hello.txt")?;
    let resource = registry.resource(&uri)?;

    resource.write_all(b"hello")?;
    let bytes = resource.read_all()?;
    assert_eq!(b"hello", bytes.as_slice());

    Ok(())
}
```

### 11.9 Provider 错误映射清单

真实 provider 应统一映射底层错误：

| 底层条件 | 建议 `FsErrorKind` |
| --- | --- |
| 404、inode 不存在、object 不存在 | `NotFound` |
| create-new 目标已存在 | `AlreadyExists` |
| key/path/URI 非法 | `InvalidPath` |
| 401 或凭据无效 | `AuthenticationFailed` |
| 403 或 ACL 拒绝 | `PermissionDenied` |
| 405 或 SDK 不支持 | `UnsupportedOperation` |
| 409 conflict | `Conflict` |
| 412 ETag 不匹配 | `PreconditionFailed` |
| 423 locked resource | `Conflict` 或 provider-specific mapping |
| 429 或服务端限流 | 根据语义映射到 `ProviderUnavailable` 或 `Timeout` |
| checksum 不匹配 | `DataCorruption` |
| 本地 stream 错误 | `Io` |

附加上下文示例：

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

### 11.10 Provider 能力声明清单

发布 provider 前，应逐项决定并测试能力：

| Capability | 需要回答的问题 |
| --- | --- |
| `hierarchical_paths` | 是真实目录结构，还是 lexical key? |
| `directories` | 后端能否表示目录或 collection? |
| `empty_directories` | 空目录没有子项时能否保留? |
| `symlinks` | 是否支持读取、复制或创建 symlink? |
| `range_read` | 是否能高效读取字节范围? |
| `append` | append 是否原生且安全? |
| `random_write` | 是否能修改任意 offset? |
| `atomic_rename` | rename 在命名空间内是否原子? |
| `atomic_replace` | 替换时是否不会暴露半写内容? |
| `conditional_write` | 是否支持 ETag、generation 或版本前置条件? |
| `server_side_copy` | 是否能不经过客户端流式转发完成复制? |
| `recursive_delete` | 是否能安全删除树? |
| `temp_file` | 是否能预留临时文件? |
| `temp_dir` | 是否能预留临时目录? |
| `temp_persist` | 临时资源是否能持久化到最终路径? |
| `temp_persist_atomic` | 临时资源持久化是否能原子完成? |
| `native_metadata` | 是否有值得保留的后端原生 metadata? |

## 12. WebDAV provider 实现建议

WebDAV provider 可按如下方式映射：

| `FileSystem` 操作 | WebDAV 方法 |
| --- | --- |
| `metadata` | `PROPFIND Depth: 0` |
| `list` | `PROPFIND Depth: 1` 或更深分页策略 |
| `open_reader` | `GET` |
| `open_writer` | `PUT`，尽可能延迟到 commit |
| `create_dir` | `MKCOL` |
| `delete` | `DELETE` |
| `rename` | `MOVE` |
| `copy` | `COPY` |

注意事项：

- `ETag` 映射到 `FileMetadata.etag`。
- `Content-Type` 映射到 `FileMetadata.content_type`。
- WebDAV 自定义属性放入 `provider_metadata`。
- `Depth` 行为要文档化，因为不同服务器差异较大。
- `MOVE` 和 `COPY` 可能只是尽力执行，不一定 atomic。
- HTTP 401、403、404、405、409、412、423、507 要仔细映射到统一错误类型。

骨架示例：

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

    // 在这里映射 PROPFIND、GET、PUT、MKCOL、DELETE、MOVE、COPY。
}
```

## 13. Local provider 实现建议

本地 provider 应把 `std::path::PathBuf` 限制在 provider 内部，对外只暴露 `FsPath`。

建议规则：

| 规则 | 说明 |
| --- | --- |
| root sandbox | 配置一个根目录，所有 `FsPath` 映射到 root 下 |
| 防逃逸 | 拒绝逃出 root 的路径 |
| symlink policy | 显式定义是否跟随 symlink |
| atomic replace | `WriteMode::ReplaceAtomic` 使用本地 atomic write |
| 错误映射 | 将 `std::io::ErrorKind` 映射到 `FsErrorKind` |
| 复用工具 | 适合复用 `qubit-local-files` 的 atomic write、递归复制和本地临时资源能力 |

路径映射 helper 示例：

```rust
use std::path::PathBuf;
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

## 14. 对象存储 provider 实现建议

OSS、S3 这类对象存储不应该伪装成完整 POSIX 文件系统。

推荐映射：

| 概念 | 映射 |
| --- | --- |
| `FsPath` | 使用规范化 `/` 分隔的 object key |
| directory | prefix 视图或可选 marker object |
| empty directory | 通常不支持，除非使用 marker object |
| `metadata` | HEAD object 或 list 结果补充 |
| `open_reader` | GET object，可支持 range GET |
| `open_writer` | PUT object 或 multipart upload |
| `rename` | copy + delete，不是原子操作 |
| `copy` | 优先 server-side copy |
| `etag` | provider ETag、generation 或 version id |
| user metadata | provider object metadata |

关键规则：

- `atomic_rename` 通常应为 false。
- 如果支持 object copy，`server_side_copy` 应为 true。
- `WriteMode::ConditionalReplace` 可映射到 generation 或 ETag precondition。
- 只要实现了 `open_writer(CreateNew)` 和 `delete()`，`TempResources` 通常就能工作。
- multipart upload 写入失败时必须 abort。

## 15. Provider 测试建议

provider 测试至少应覆盖：

1. URI 解析和 provider 注册。
2. 基础 write、read、metadata、exists、list、delete。
3. not found、already exists、permission denied、authentication failed、unsupported operation 的错误映射。
4. `WriteMode::CreateNew`、`CreateOrTruncate`、`ReplaceAtomic` 和 provider 支持的 conditional mode。
5. 如果 `range_read = true`，覆盖 `ReadOptions` 的 range 行为。
6. `RenameOptions.atomic = Required` 的行为。
7. `CopyOptions` 的 conflict policy 和 server-side preference。
8. 如果 capabilities 声明支持，测试 `TempResources::create_file()` 和 `create_dir()`。
9. `ManagedTempFile::persist()`、`cleanup()`、`keep()` 行为。
10. 大目录 list，包括分页。
11. metadata preservation 和 provider metadata。
12. 写入失败、copy-delete fallback 失败后的清理。

建议把最小合约测试写成接收 `Arc<dyn FileSystem>` 的测试函数，这样 Local、WebDAV、OSS、Memory provider 可以复用同一套测试。

## 16. 最佳实践

| 主题 | 建议 |
| --- | --- |
| Path | 对外统一使用 `FsPath`，provider 内部再转换为 native path 或 key |
| URI | `FsUri` 用于定位 provider 和初始化，不要每个操作都传完整 URI 字符串 |
| Secret | 不要把 secret 放入 `FsUri.query`，使用 `CredentialRef` 或 provider 安全配置 |
| Capabilities | 依赖高级语义前先检查 capabilities |
| Unsupported | 不支持就返回 `UnsupportedOperation`，不要假装支持 |
| Writing | writer 必须 commit 或 abort |
| Temp resources | 用 `cleanup()`、`persist()`、`keep()` 明确所有权转移 |
| Copy | 用 `CopyOutcome.method` 和 `CopyStats` 做审计和诊断 |
| Error | 尽量保留 operation、path、target、provider、source error |
| Provider crate | 具体实现放在独立 crate，通过 `qubit-spi` 注册 |

## 17. 当前限制

当前 `qubit-fs` 只提供核心抽象，尚未提供：

- 内置 local provider
- 内置 WebDAV provider
- 内置 OSS/S3/HDFS provider
- async trait
- 全局自动 provider discovery
- 跨 provider copy orchestration
- credential resolution 实现

这些能力应由独立后端 crate 或后续扩展层提供。

## 18. 推荐接入路径

应用开发者建议按以下步骤接入：

1. 引入 `qubit-fs` 和一个或多个 provider crate。
2. 通过 `FileSystemRegistry::builder()` 创建 `FileSystemRegistryBuilder`。
3. 显式注册 provider，然后调用一次 `build()`。
4. 把 URI 字符串解析成 `FsUri`，再用 `FileSystemRegistry::resource()` 解析资源。
5. 优先使用 `FileResource` 执行资源导向操作；底层实现仍然使用 `FileSystem` + `FsPath`。
6. 依赖高级行为前检查 capabilities。
7. 按 `FsErrorKind` 处理错误，不直接依赖 provider-native error。

provider 作者建议按以下步骤实现：

1. 为后端实现 `FileSystem`。
2. 按需实现 `FileReader`、`FileWriter`、`DirectoryStream`。
3. 准确声明 `FileSystemCapabilities`。
4. 将后端原生错误映射为 `FsErrorKind`。
5. 实现 `ServiceProvider<FileSystemSpec>`。
6. 暴露 `register_provider()` helper。
7. 编写基于 `Arc<dyn FileSystem>` 的合约测试。
8. 文档化后端专属语义，尤其是 atomicity、directory、metadata、credential。
