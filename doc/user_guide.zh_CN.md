# Qubit FS 用户指南

`qubit-fs` 是应用代码与文件系统 provider 之间的公共契约，适用于本地文件系统、
对象存储、云盘、远程协议、分布式文件系统、内存实现和 provider-specific 存储服务。

核心 crate 不包含具体后端。增加新后端不需要增加枚举分支，也不需要修改应用逻辑；
应用只需在组装阶段注册所需 provider。

## 安装

```toml
[dependencies]
qubit-fs = "0.2"
```

文件字节流使用 `qubit-io` 的 trait。Provider 实现还会使用 `qubit-spi`，通常也会
使用 `qubit-metadata`。

## 对象模型

文件系统 trait 层次如下：

```rust
use qubit_fs::{
    AsyncFileSystem,
    FileSystem,
    FileSystemProperties,
};
```

- `FileSystemProperties` 包含 `info()`、`capabilities()` 与 `limits()`。三者都是
  构造时确定的稳定快照，getter 绝不能触发 I/O；
- `FileSystem: FileSystemProperties` 包含同步操作；
- `AsyncFileSystem: FileSystemProperties` 包含运行时无关的异步操作，方法名统一以
  `_async` 结尾。

同一个后端类型可以实现其中一个操作 trait，也可以同时实现两者。同步与异步 registry
彼此独立，因为 provider 可能只支持一种模式。

`stat()` 与 `stat_async()` 是实时操作，可以触发远程 I/O。它们和 `info()` 不同，
也不同于打开句柄时携带的可选 metadata 快照。两者都检查最终路径项本身：最终项为
符号链接时返回 `FileKind::Symlink`，不会跟随链接。

## URI 与 Provider-Local Path

### `FsUri`

`FsUri` 是传输与 provider 选择表示。它会保留核心层不应擅自解释的信息：

- 原始 percent-encoded path，包括 `%2F` 这样的编码分隔符；
- query pair 的顺序与重复 key；
- 是否使用了 `//` 层次形式，包括 `file:/tmp/a` 与 `file:///tmp/a` 的区别；
- 可选 authority。

Fragment、非法 percent encoding、控制字符、password 和 credential-like query key
都会被拒绝，其中包括 `x-amz-signature` 等 signed-URL 字段。Query 是非敏感
provider option，不是凭据通道。Raw path 使用 RFC 3986 encoded syntax；非 ASCII、
空白、`?` 与 `#` 必须 percent encode。Component builder 还会拒绝序列化后产生歧义的
authority/path 组合。

```rust
use qubit_fs::FsUri;

let uri = FsUri::parse(
    "object://bucket/a%2Fb?tag=first&tag=second",
)?;

assert_eq!("object", uri.scheme().as_str());
assert_eq!("/a%2Fb", uri.path().as_encoded());
assert_eq!(vec!["first", "second"], uri.query().get_all("tag"));
# Ok::<(), qubit_fs::FsError>(())
```

`FsUriPath::decode()` 只执行语法层 percent decode，得到 UTF-8 文本，不会把文本解释
为 `FsPath`。Encoded separator、dot segment、object key、drive identifier 和
authority 的语义因后端而异，转换责任属于选中的 provider。

### `FsPath`

`FsPath` 是已经在某个已配置文件系统内部解释过的路径。`parse()` 使用 normalized
hierarchical semantics；`parse_literal()` 保留 provider-specific object-key 文本。

```rust
use qubit_fs::{
    FsName,
    FsPath,
    RelativeFsPath,
};

let base = FsPath::parse("/work")?;
let child = base.child(&FsName::parse("result.bin")?);
let nested = base.join_relative(
    &RelativeFsPath::parse("2026/july/report.csv")?,
);

assert_eq!("/work/result.bin", child.as_str());
assert_eq!("/work/2026/july/report.csv", nested.as_str());
# Ok::<(), qubit_fs::FsError>(())
```

`FsName` 与 `RelativeFsPath` 无法表示绝对路径或向上逃逸。`TempDir::child()` 等 API
使用这些类型，使规范 `/` namespace 中的词法路径逃逸在类型层面无法表达。
Hierarchical provider 仍必须逐 component 解码 native path，并拒绝解码后引入目标
平台 native separator、root 或 prefix 的 component。

不要把 `std::path::Path` 当作跨 provider 的统一模型。本地 provider 可以在应用自身
平台规则与 sandbox 规则后，在内部进行转换。

## Registry 集成

核心 crate 有意不提供 provider 发现或 credential reference。需要这些能力的应用应
依赖 [`qubit-fs-registry`](https://crates.io/crates/qubit-fs-registry)，该 crate 提供
`FileSystemRegistry`、`AsyncFileSystemRegistry`、`FileSystemConfig` 与
`CredentialRef`。Provider 会接收完整配置，解码 provider-local `FsPath`，并返回
不含 credential 的 canonical URI。同步和异步注册示例请参阅该 crate 的 README。

## 文件身份与 Metadata

`FileReader`、`FileWriter`、`AsyncFileReader` 与 `AsyncFileWriter` 是文件句柄，
不是任意字节流的别名。Provider 必须用已经打开的流和 `OpenedFileInfo` 显式构造。

每个打开句柄都有固定 `FileLocation`：

- 已配置文件系统的 `FileSystemId`；
- 打开时捕获的 provider-local path；
- 可选 canonical、secret-free URI。

`OpenedFileInfo::metadata()` 是可选的 open-time 快照。只有在打开过程中本来就拿到
metadata 时才应填充，provider 不应为了填字段额外发起远程 `stat`。需要当前状态时，
调用 `FileSystem::stat()` 或 `AsyncFileSystem::stat_async()`。
可扩展的 `FileMetadata::user_metadata` 与 `provider_metadata` 字段使用
`NonSensitiveMetadata`，provider 必须通过受校验转换或对应 builder method 构造。

## 同步读取

内容可以安全放入内存时，可使用 `FileSystemExt::read_all()` 或
`FileResource::read_all()`。流式读取时，`FileReader` 实现
`qubit_io::Input<Item = u8>`。

```rust
use qubit_fs::{
    FileResource,
    FsError,
    FsOperation,
    FsResult,
    ReadOptions,
};
use qubit_io::Input;

fn read_prefix(resource: &FileResource) -> FsResult<Vec<u8>> {
    let mut reader = resource.open_reader(ReadOptions::default())?;
    let mut buffer = [0_u8; 4096];
    let count = reader.read(&mut buffer).map_err(|error| {
        FsError::from_io(error, FsOperation::Read)
            .with_path(resource.path().clone())
    })?;
    Ok(buffer[..count].to_vec())
}
```

`ReadOptions` 描述 byte range、version condition 与 checksum policy。调用方要求的
option 无法满足时，provider 必须返回明确错误，不能静默忽略。

## 同步写入

`FileWriter` 实现 `qubit_io::Output<Item = u8>`，同时也是 provider publication
session。写入字节后，必须显式调用 `commit()` 或 `abort()`。

```rust
use qubit_fs::{
    AtomicityRequirement,
    FileResource,
    FsError,
    FsOperation,
    FsResult,
    WriteDisposition,
    WriteOptions,
    WriteOutcome,
};
use qubit_io::Output;

fn replace(
    resource: &FileResource,
    bytes: &[u8],
) -> FsResult<WriteOutcome> {
    let options = WriteOptions {
        create_parent: true,
        disposition: WriteDisposition::CreateOrReplace,
        atomicity: AtomicityRequirement::Required,
        ..WriteOptions::default()
    };
    let mut writer = resource.open_writer(&options)?;
    if let Err(error) = writer.write_fully(bytes) {
        let _ = writer.abort();
        return Err(FsError::from_io(error, FsOperation::Write)
            .with_path(resource.path().clone()));
    }
    writer.commit()
}
```

`commit(&mut self)` 有意不消费 handle。同步 provider 返回带状态的
`WriteFailure`：`Retryable` 让 writer 保持 Open；`NotPublished` 与 `Published`
进入只能显式清理的终态；`Indeterminate` 表示无法确认 publication 是否发生。
只有 `Retryable` 允许再次 commit。

Open、NotPublished 或 Published 的同步 writer 在 drop 时会 best-effort abort。
Indeterminate writer 可能已经发生 publication 或 cleanup，因此绝不自动 abort。
Drop 无法返回清理错误，需要确认清理的调用方必须自己调用 `abort()`。

## 异步操作

异步文件系统操作返回 `FsFuture`，核心 crate 不绑定 Tokio、`futures-io` 或其他
executor。Open 之所以是异步，是因为它可能包含认证、连接、请求协商或 multipart
初始化。成功 open 后返回已经初始化的异步句柄。

```rust
use qubit_fs::{
    AsyncFileResource,
    FsError,
    FsOperation,
    FsResult,
    ReadOptions,
};
use qubit_io::AsyncInput;

async fn read_prefix(
    resource: &AsyncFileResource,
) -> FsResult<Vec<u8>> {
    let mut reader =
        resource.open_reader_async(ReadOptions::default()).await?;
    let mut buffer = [0_u8; 4096];
    let count = reader.read_async(&mut buffer).await.map_err(|error| {
        FsError::from_io(error, FsOperation::Read)
            .with_path(resource.path().clone())
    })?;
    Ok(buffer[..count].to_vec())
}
```

Async writer 实现 `AsyncOutput<Item = u8>`：

```rust
use qubit_fs::{
    AsyncFileResource,
    FsError,
    FsOperation,
    FsResult,
    WriteOptions,
    WriteOutcome,
};
use qubit_io::AsyncOutput;

async fn write(
    resource: &AsyncFileResource,
    bytes: &[u8],
) -> FsResult<WriteOutcome> {
    let mut writer =
        resource.open_writer_async(WriteOptions::default()).await?;
    if let Err(error) = writer.write_fully_async(bytes).await {
        let _ = writer.abort_async().await;
        return Err(FsError::from_io(error, FsOperation::Write)
            .with_path(resource.path().clone()));
    }
    writer.commit_async().await
}
```

异步 drop 无法等待远程清理。`AsyncFileWriteSession::cancel_on_drop` 只能执行非阻塞
本地取消，并且只处理确定仍为 Open 的 writer。commit/abort future 一旦被 poll，若在
完成前被 drop，writer 就进入 `Indeterminate`；从未 poll 的 future 不改变状态。
Indeterminate writer 不执行自动取消。只要需要确认清理，就必须 await
`abort_async()`。

对确定可放入内存的资源，`AsyncFileSystemExt::read_all_async()`、
`write_all_async()` 以及 `AsyncFileResource` 上的同名便利方法提供整体读写 helper
的异步对应形式。

## Listing 与 Bound Resource

`DirectoryStream::next_entry()` 与
`AsyncDirectoryStream::next_entry_async()` 支持 provider 内部增量分页。明确接受完整
listing 内存开销时，可使用 `DirectoryStreamExt::collect_entries()` 或
`AsyncDirectoryStreamExt::collect_entries_async()`。

`FileResource` 与 `AsyncFileResource` 把 decoded path 绑定到 owning filesystem，
提供 stat、exists、list、open、create、delete、rename 和 copy 便捷操作。
`FsPath` 自身不携带 backend 状态，因此仍然轻量、可比较、易测试。

## Capability、Requirement 与 Outcome

Capability 是类型化保证：

```rust
use qubit_fs::{
    FileSystem,
    FileSystemCapability,
};

fn supports_atomic_rename(fs: &dyn FileSystem) -> bool {
    fs.capabilities().contains(FileSystemCapability::AtomicRename)
}
```

Capability 快照描述当前已配置文件系统保证什么，而不是某个泛型 provider 偶尔可能
尝试什么。独立的 `FileSystemProperties::limits()` 快照携带稳定配置限制，并明确区分
未知、不适用和无界的限制维度。

`ListOptions::page_size` 是 hint。绑定资源会把它收敛到有限的
`max_list_page_entries`；直接 provider 也必须在 I/O 前执行相同收敛。未提供 hint 时，
provider 自选的 page 仍不得超过该有限上限。

`AtomicityRequirement` 有三种语义：

- `Required`：成功必须满足原子性；无法保证时，在副作用前失败；
- `Preferred`：优先原子方法，但允许非原子成功，且必须报告；
- `NotRequired`：调用方不要求原子性，但 provider 仍可使用原子方法。

成功的 `WriteOutcome`、`RenameOutcome`、`CopyOutcome` 与 `PersistOutcome` 会报告
`AchievedAtomicity` 和实际 method。请求策略与完成结果有意使用不同类型。

Provider 应在 mutation 前校验 Required guarantee。`ReadOptions`、`WriteOptions`、
`DeleteOptions`、`RenameOptions`、`CopyOptions` 与 `PersistOptions` 都提供
`validate_against()` preflight。缺少 range、conditional、checksum、append、原子
publication、recursive delete 或 server-side copy 保证时，错误会携带对应的 typed
capability。
`WriteOptions::validate()` 还会拒绝自身矛盾的请求，例如同时指定 `CreateNew` 与
`IfMatch`。

## 错误

`FsError` 携带：

- provider-neutral `FsErrorKind`；
- `FsOperation`；
- 可选 source path 与 target path；
- 可选 provider identity；
- 可选 required capability；
- 原始 source error。

Open、metadata、lifecycle 与 namespace 操作返回 `FsError`。字节传输方法遵循
`qubit-io`，返回 `std::io::Error`。`FsError::into_io_error()` 与
`FsError::from_io()` 用于跨越这条边界，并尽可能保留底层 source。

`exists()` 只有在明确 `NotFound` 时才返回 `Ok(false)`。认证、权限、超时与网络错误
仍然是错误。

## 临时资源

不存在跨后端通用的默认临时目录。本地文件系统、对象存储、云盘与分布式文件系统没有
共同的安全 namespace、生命周期或 publication strategy。因此 provider 必须显式
实现 `create_temp_file` / `create_temp_dir` 或异步对应方法，并声明相应 capability。

临时句柄拥有 cleanup responsibility，提供：

- `cleanup`：删除 source 并确认清理；
- `keep`：把责任转交调用方；
- `persist`：发布到最终路径，但不消费 handle。

`TempDir::child()` 接收 `FsName`，`descendant()` 接收 `RelativeFsPath`，从而防止
绝对路径替换与词法 `..` 逃逸。

Persist 失败是类型化的：

| 状态 | 含义 | 调用方动作 |
| --- | --- | --- |
| `NotPublished` | target 未发布，handle 仍拥有 source | 修正后重试或清理 |
| `PublishedSourceRetained` | target 已发布，source 仍需清理 | 不要再次发布；清理 source |
| `Indeterminate` | provider 无法确认 source/target 最终状态 | 外部对账；禁止自动清理或重试 |

`persist(&mut self, ...)` 会保留 handle，因此失败时不会丢失临时 session。同步 drop
只对确定 owned 状态执行 best-effort cleanup。异步 cleanup、keep 或 persist future
一旦被 poll，若在完成前被 drop，handle 就进入 `Indeterminate` 并禁用自动取消；从未
poll 的 future 不影响生命周期。远程清理必须显式 await。

## 实现 Provider

同步 provider 是自描述的
`qubit_spi::ProviderDefinition<FileSystemSpec>`，输出
`FileSystemResolution<dyn FileSystem>`。异步 provider 独立实现
`AsyncFileSystemProvider`。

Provider 应当：

1. 校验完整 `FileSystemConfig`；
2. 通过外部 secret source 解析 `CredentialRef`；
3. 按 provider 语义解码 raw URI path；
4. hierarchical native path 必须逐 component 转换，并拒绝解码后产生的 separator、
   root 或 prefix；
5. 构造 immutable `FileSystemInfo`、`FileSystemCapabilities` 与
   `FileSystemLimits` 快照；
6. 返回 configured filesystem、decoded `FsPath` 与安全 canonical URI；
7. 用稳定 identity 创建显式 file/lifecycle handle；
8. 在副作用前执行有限 list-page 上限并拒绝无法满足的 guarantee；
9. 报告实际 publication method、atomicity 与 partial progress；
10. 为错误附加 operation、path、provider、capability 与 source context。

`FileSystemInfo::with_provider_metadata()` 会拒绝顶层、string map 与 JSON object
内部的 credential-like key，因为该快照可出现在 Debug 输出中。File metadata、
options 与 outcome diagnostics 也由 `NonSensitiveMetadata` 强制执行同一不变量，自动
Debug 只暴露 key，不暴露 value。所有 secret value 都必须在核心模型之外解析。
`FsError` message 也必须预先清洗；它的 `Debug` 与 `Display` 不展开保留的 source
error，显式诊断仍可通过 `Error::source()` 访问。

可选操作的默认 trait 方法返回 `UnsupportedCapability`。Provider 只应 override 自己
真正支持的操作。
