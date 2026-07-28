# Qubit FS 用户指南

`qubit-fs` 将应用层 filesystem facade 与 provider 层契约分开。核心 crate 不内置
后端，也不选择异步运行时；provider 发现、凭据和配置属于
[`qubit-fs-registry`](https://crates.io/crates/qubit-fs-registry)。

## 应用与 provider 边界

应用通过 provider 实现创建具体 facade，并用逻辑 `Path` 表示每个资源。provider trait、
request、session 和 envelope 只位于 `qubit_fs::spi`。

```rust,ignore
use qubit_fs::{FileSystem, Path, ReadOptions};
use qubit_fs::spi::FileSystemSpi;

fn inspect<S: FileSystemSpi + 'static>(provider: S) -> qubit_fs::FsResult<()> {
    let filesystem = FileSystem::from_spi(provider)?;
    let path = Path::parse("/reports/2026/summary.csv")?;
    let _metadata = filesystem.stat(&path)?;
    let _reader = filesystem.open_reader(&path, ReadOptions::default())?;
    Ok(())
}
```

`FileSystemProperties` 是构造时不可变快照，包含 provider identity、capability、limit
和 path constraint。`stat`、list、open、create、delete、copy、rename 与临时资源操作
都可能执行 I/O。

## 路径与 URI

`Path` 表示经过验证的逻辑资源名。`Path::parse` 使用 hierarchical 验证；配置的
`PathSemantics` 允许时，`Path::parse_literal` 保留 provider-specific 拼写。
`RelativePath` 和 `PathComponent` 可构造安全的后代路径，且不能向上逃逸。

`Uri` 与 `ConnectionUri` 保留 RFC 3986 的词法结构，但拒绝 fragment、userinfo 和含
credential 的 query field。它们是传输/配置值，不能取代 provider 对逻辑 `Path` 的验证。
`UserMetadata` 同样拒绝 credential-like key，且 `Debug` 不显示 value。

这些 URI 凭据边界会同时使用内置保守分类与应用规则；进程级脱敏策略中的 allow 规则不能
让含凭据 URI 变为有效，也不能通过普通格式化暴露它。

## 同步 I/O

直接从 `FileSystem` 打开 reader 或 writer。句柄在 `OpenedFileInfo` 中保留 provider-opened
identity，调用 `info()` 不会触发额外 provider I/O。

```rust,ignore
use qubit_fs::{FileSystem, Path, WriteOptions};
use qubit_io::Output;

fn replace(fs: &FileSystem, path: &Path, bytes: &[u8]) -> qubit_fs::FsResult<()> {
    let mut writer = fs.open_writer(path, WriteOptions::default())?;
    writer.write_fully(bytes).map_err(|error| {
        qubit_fs::FsError::from_io(error, qubit_fs::FsOperation::Write)
    })?;
    writer.commit().map_err(|failure| failure.into_error())?;
    Ok(())
}
```

`FileWriter::commit` 返回带状态的 `WriteFailure`，用于区分 retryable、not-published、
published 和 indeterminate；需要确认清理时保留 writer 并调用 `abort`。
`DirectoryStream::next_entry` 是增量读取，应用必须自行限制收集数量。

`FileSystem::copy` 和 `FileSystem::rename` 分别返回 `CopyFailure` 与 `RenameFailure`。
Required atomicity/durability 会在副作用前预检，成功 outcome 会报告实际达到的保证。

## 异步 I/O

`AsyncFileSystem` 提供与同步 facade 对应的 runtime-neutral 方法；provider 契约是
`spi::AsyncFileSystemSpi`，调用方可在任意运行时中 await facade 方法。

```rust,ignore
use qubit_fs::{AsyncFileSystem, Path, ReadOptions};

async fn inspect(fs: &AsyncFileSystem, path: &Path) -> qubit_fs::FsResult<()> {
    let _metadata = fs.stat(path).await?;
    let _reader = fs.open_reader(path, ReadOptions::default()).await?;
    Ok(())
}
```

`AsyncFileSystem::begin_copy` 会创建 `AsyncCopyOperation`。执行 future 一旦被 poll，
若在完成前被 drop，操作会记录 indeterminate 状态且不启动额外 cleanup I/O。需要确认
async writer 或临时资源的清理时必须显式 await。

## 临时资源

`create_temp_file` 和 `create_temp_directory` 返回 facade-owned handle。`TempFile`、
`TempDirectory`、`AsyncTempFile` 与 `AsyncTempDirectory` 都提供 `cleanup`、`keep`、
`persist` 的显式生命周期操作。persist failure 保留 `NotPublished`、
`PublishedSourceRetained` 或 `Indeterminate`，调用方可据此重试、清理或对账。

## 错误与保证

`FsError` 包含 error kind、operation 与可用的逻辑 source/target context。`exists` 只有
明确收到 not-found 才返回 `false`；权限、认证、网络和超时仍是错误。只要 facade 能在
本地判断某项保证不可满足，就会在副作用前完成 capability preflight。
