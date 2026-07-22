# Qubit FS 文件系统抽象层设计

本文定义 `qubit-fs` 的长期架构、核心语义与 provider 实现约束。它描述当前公开
API，而不是某个具体后端或一次性迁移过程。

## 1. 设计目标

`qubit-fs` 的目标不是把所有存储伪装成 POSIX 文件系统，而是为应用层提供一个稳定、
可组装且语义诚实的公共层：

1. 应用只依赖文件系统抽象，不依赖本地、远程、云端、云盘或分布式后端；
2. 新 provider 可独立发布和注册，不修改核心 crate；
3. 同步与异步应用获得形态一致的对象和操作；
4. 后端差异通过 capability、option、outcome 与 error 显式表达；
5. 无法满足的强语义必须失败，不能静默降级；
6. URI、凭据、临时资源和部分成功具有明确安全边界；
7. 核心层保持小而实用，不绑定平台路径模型或异步运行时。

非目标包括：

- 提供一个“所有操作必定支持”的最大接口承诺；
- 把对象存储 key 强行解释成目录路径；
- 在核心 crate 中内置所有 provider；
- 为所有后端猜测统一的临时目录；
- 在 URI 中携带 secret；
- 绑定 Tokio、`futures-io` 或某个 executor；
- 用标准库 `Read` / `Write` 定义文件句柄身份。

## 2. 分层与依赖方向

```text
应用层
  │  只依赖 provider-neutral 对象
  ▼
qubit-fs
  ├─ 领域模型：URI、path、metadata、capability、option、outcome、error
  ├─ 操作接口：FileSystem / AsyncFileSystem
  ├─ 文件句柄：FileReader / FileWriter / Async*
  ├─ 生命周期：writer、directory stream、temporary resource
  └─ 组装接口：registry、provider、FileSystemConfig
       │
       ├─ qubit-io：同步与运行时无关异步流
       ├─ qubit-spi：同步 provider 注册和选择
       └─ qubit-metadata：可扩展非敏感 metadata
             │
             ▼
        独立 backend crate
```

依赖只向下。Provider 可以依赖平台 SDK、网络协议或 runtime adapter；核心层不反向
依赖具体 provider。

## 3. 三种不同的“身份”

文件系统抽象容易混淆 provider、configured filesystem 和 resource。它们必须分开：

### 3.1 Provider

Provider 是工厂和协议适配器。它负责：

- 声明 provider id、alias 和选择优先级；
- 校验完整配置；
- 解析 credential reference；
- 解释 URI authority、path 与 query；
- 构造 configured filesystem；
- 返回 provider-local path 与安全 canonical URI。

Provider 本身不是一个文件系统对象。

### 3.2 Configured filesystem

`FileSystem` 或 `AsyncFileSystem` 是一次配置完成后的操作对象。例如，同一个 S3
provider 可以构造不同 region、endpoint、bucket scope 或 credential profile 的多个
filesystem。

`FileSystemInfo::id()` 标识该 configured filesystem，而
`FileSystemInfo::provider_id()` 标识创建它的 provider。两者不可混用。

### 3.3 Resource 与 opened handle

`FileResource` / `AsyncFileResource` 把一个 decoded `FsPath` 绑定到 owning
filesystem。`FileReader` / `FileWriter` 则代表一次已经打开的文件会话，并携带打开时
固定的 `OpenedFileInfo`。

这种拆分避免把 backend 状态塞进 `FsPath`，也避免把任意字节流误认为文件。

## 4. URI 与 Path 语义

### 4.1 第一原则

URI 是跨边界定位与 provider 选择表示；path 是 provider 解释后的内部资源表示。
如果核心层提前解码或规范化 URI，就会不可逆地破坏 provider 语义。

因此：

```text
FsUri（传输表示）
  └─ provider 选择、校验和解码
       ├─ configured filesystem
       ├─ FsPath（provider-local 表示）
       └─ canonical FsUri（安全、可记录）
```

### 4.2 `FsUri`

`FsUri` 包含：

- `FsScheme`：小写、经过语法校验，用于默认 provider selection；
- 可选 `FsAuthority`：host/bucket/namespace/endpoint、port 和非敏感 username
  hint；
- `authority_present`：保留 `//` 是否出现；
- `FsUriPath`：经过语法校验但未按 provider 语义解码的 raw encoded path；
- `FsUriQuery`：decoded、ordered、允许 duplicate key 的非敏感 pair。

以下区别必须保留：

```text
file:/tmp/a      authority component 不存在
file:///tmp/a    authority component 存在，但 authority 为空
object://b/a%2Fb encoded "/" 仍属于 path 数据，核心层不得拆分
```

`FsUriPath` 校验 RFC 3986 path 字符、percent encoding、decoded UTF-8 与控制字符；
非 ASCII、空白、`?` 与 `#` 必须先编码。它不：

- 解码 `%2F`；
- 规范化 dot segment；
- 把 repeated separator 合并；
- 推断 object-key 或 hierarchy。

这些决定属于 provider。

`FsUriQuery` 保留 encounter order 与 duplicate key，因为签名、路由和 provider option
可能依赖它们。Display 会输出 canonical percent encoding。

### 4.3 Credential 边界

URI 是日志、配置和错误上下文中可能出现的对象，因此必须 secret-free：

- authority 禁止 password；
- query 禁止 password、token、access key、secret key、signed-URL signature 等
  credential-like key，包括带 provider 前缀的形式；
- fragment 禁止；
- `FileSystemConfig::with_options` 对普通 metadata option 执行相同敏感 key 检查，
  并递归检查 string map、JSON object 以及数组内 object；
- secret 只能通过 `CredentialRef` 指向外部 credential source。

这个结构化校验由 `NonSensitiveMetadata` 统一承载，并用于 config options、
filesystem/file metadata、write/directory metadata 以及各类 outcome diagnostics。
其内部 `Metadata` 不提供可变访问，构造后始终保持校验不变量；Debug 只输出 key，
不会自动输出 value。

该边界按结构化 key 判断。普通 scalar string 的内容无法可靠分类，provider 必须保证
其非敏感；任何 secret value 都只能位于 `CredentialRef` 指向的外部系统。

`CredentialRef` 本身只保存 default chain、profile、环境变量名或外部 provider id，
不保存 secret value。Debug 输出也不展开凭据。

Provider 返回的 canonical URI 必须遵守相同边界，才能安全地附加到 `FileLocation`。

### 4.4 `FsPath`

`FsPath` 表示一个 configured filesystem 内部的 UTF-8 provider-local path：

- `parse` / `parse_normalized` 用于 hierarchical semantics；
- `parse_literal` 用于 object key 或 provider-specific semantics；
- `PathSemantics` 在 `FileSystemInfo` 中声明
  `Hierarchical`、`ObjectKey` 或 `ProviderSpecific`。

Hierarchical parse 会移除 repeated separator 和 `.`，解析 `..`，并拒绝越过根。
Literal parse 只执行基本安全校验，保留 repeated separator、`.` 与 `..` 文本。

为了安全组合路径，提供两个更窄的类型：

- `FsName`：一个非空 component，在规范 `/` namespace 中不能包含 separator、`.` 或
  `..`；
- `RelativeFsPath`：非空、normalized、不能是 absolute，也不能逃逸 relative root。

`FsPath::child`、`join_relative`、`TempDir::child` 和 `descendant` 使用这些类型。
原始字符串 convenience `FsPath::join` 也会先解析为 `RelativeFsPath`，绝对路径不能
替换 base。

`FsUriPath::decode()` 只执行 URI percent decode，不负责把结果解释为 `FsPath`。
`NativePathCodec` 也只转换 opaque fragment，不解释 root、prefix 或 separator。
Hierarchical provider 必须逐 component 转换，并拒绝解码后引入目标平台 native
separator、root 或 prefix 的 component。

## 5. FileSystem Trait 层次

### 5.1 公共父 trait

```rust
pub trait FileSystemProperties: Send + Sync {
    fn info(&self) -> &FileSystemInfo;
    fn capabilities(&self) -> FileSystemCapabilities;
    fn limits(&self) -> &FileSystemLimits;
}
```

`info()`、`capabilities()` 和 `limits()` 都是 construction-time local snapshot：

- getter 不触发本地或远程 I/O；
- 结果在对象生命周期内稳定；
- 需要 remote probe 的 provider 应在 construction/open configuration 阶段完成；
- 结果描述当前 configured filesystem，而不是 provider 的理论上限。

`FileSystemInfo::with_provider_metadata` 会把输入转换成 `NonSensitiveMetadata`，拒绝
顶层、string map 与 JSON object 内部的 credential-like key，保证这个可被 Debug
和日志观察的快照不成为 secret 通道。

这个父 trait 只描述文件系统对象本身，不冒充完整操作接口。

### 5.2 同步接口

```rust
pub trait FileSystem: FileSystemProperties {
    fn stat(&self, path: &FsPath) -> FsResult<FileMetadata>;
    fn open_reader(
        &self,
        path: &FsPath,
        options: ReadOptions,
    ) -> FsResult<FileReader>;
    fn open_writer(
        &self,
        path: &FsPath,
        options: WriteOptions,
    ) -> FsResult<FileWriter>;
    // list/create/delete/rename/copy/temp ...
}
```

没有 `SyncFileSystem` 名称，因为 Rust API 惯例中无前缀版本就是同步版本。

### 5.3 异步接口

```rust
pub trait AsyncFileSystem: FileSystemProperties {
    fn stat_async<'a>(
        &'a self,
        path: &'a FsPath,
    ) -> FsFuture<'a, FileMetadata>;
    fn open_reader_async<'a>(
        &'a self,
        path: &'a FsPath,
        options: ReadOptions,
    ) -> FsFuture<'a, AsyncFileReader>;
    fn open_writer_async<'a>(
        &'a self,
        path: &'a FsPath,
        options: WriteOptions,
    ) -> FsFuture<'a, AsyncFileWriter>;
    // list/create/delete/rename/copy/temp ..._async
}
```

异步方法添加 `_async`，因此同一 struct 可以同时实现两套 trait，而 method call 不会
产生名字歧义。

Open 是异步操作，而不只是“同步返回一个异步 reader”。原因是 open 本身可能需要：

- DNS、连接和认证；
- 远程 metadata 或 precondition 校验；
- range request 初始化；
- multipart upload 或 staging session 创建；
- provider SDK 的异步资源分配。

`open_reader_async().await` 成功后返回已经打开的 `AsyncFileReader`；之后字节读取仍然
是异步的。这和同步 `open_reader()` 成功后返回已经打开的 `FileReader` 具有一致语义。

### 5.4 `stat` 与 metadata snapshot

三个概念必须严格区分：

| 概念 | 是否 I/O | 是否实时 | 用途 |
| --- | --- | --- | --- |
| `FileSystemInfo` | getter 不 I/O | 构造时固定 | filesystem/provider 身份与 path semantics |
| `FileSystemCapabilities` | getter 不 I/O | 构造时固定 | 可选保证 |
| `stat` / `stat_async` | 可以 I/O | 当前观察 | 资源 metadata |
| `OpenedFileInfo::metadata` | open 已有时附带 | open-time snapshot | 避免重复 lookup |

Provider 不应为了填充 `OpenedFileInfo::metadata` 额外发起一次 `stat`。
`stat` 与 `stat_async` 都检查最终路径项本身，不跟随最终符号链接；最终项为符号链接时
返回 `FileKind::Symlink`。

## 6. 字节流与文件句柄

### 6.1 为什么不用 `std::io::Read` / `Write`

`qubit-fs` 的底层字节流使用：

- `qubit_io::Input<Item = u8>`；
- `qubit_io::Output<Item = u8>`；
- `qubit_io::AsyncInput<Item = u8>`；
- `qubit_io::AsyncOutput<Item = u8>`。

原因是：

1. 应用可在同一抽象上组合 buffering、limit、count、checksum、binary 与 text
   adapter；
2. 同步与异步流共享一致的 item-oriented 模型；
3. 异步核心只依赖 `Poll`，不绑定 runtime；
4. 标准库或生态流可以在 `qubit-io` 边界适配；
5. 文件系统层不需要复制通用 stream 能力。

### 6.2 为什么任意 Input 不是 FileReader

`Input<u8>` 只表示“可以读取字节”。它可能来自内存、socket、解码器、压缩流或随机
生成器，与文件没有必然关系。

`FileReader` 还必须表示：

- 它由某个 configured filesystem 打开；
- 它对应一个固定 `FileLocation`；
- 它可能携带 open-time metadata snapshot；
- 它的错误与生命周期来自文件 provider。

因此不存在 `impl<I: Input<u8>> FileReader for I` 之类 blanket conversion。Provider
必须显式调用 `FileReader::new(input, opened_info)`。`AsyncFileReader` 同理。

### 6.3 固定身份

`FileLocation` 包含：

- `FileSystemId`；
- open-time provider-local `FsPath`；
- 可选 canonical credential-free URI。

即使资源后来 rename，已打开 handle 的 location 也不会被悄悄改写。它描述“这个
handle 当初打开的对象”，不是对 namespace 的实时反向查询。

Registry-bound resource 在 open 后会把 provider-local identity 补充为 registry
解析得到的 canonical location，但不会改变已捕获的 metadata snapshot。

### 6.4 Type erasure

公开 `FileReader`、`FileWriter`、`AsyncFileReader`、`AsyncFileWriter` 是 concrete
type-erased handle，而不是让每个应用传播 provider-specific 泛型。Provider-side
extension point 则是：

- `FileWriteSession`；
- `AsyncFileWriteSession`；
- `DirectoryStreamSession`；
- `AsyncDirectoryStreamSession`；
- temporary session trait。

这样应用 API 简洁，provider 仍可使用自己的实现类型。

## 7. Capability、Requirement 与 Outcome

### 7.1 Capability 不是布尔字段集合

`FileSystemCapability` 是稳定、可扩展的 typed identifier，包括 read、range read、
conditional read、checksum validation、append、conditional write/delete、atomic rename、
server-side copy、temporary resource 等。

`FileSystemCapabilities` 是纯 capability set；`FileSystemLimits` 由
`FileSystemProperties::limits()` 独立返回，并明确表示未知、不适用、无界或有限上限。
错误可以通过 `required_capability` 指回具体缺失保证，而不依赖模糊字符串。

`max_list_page_entries` 是 provider-native page 的有限上限。
`ListOptions::page_size` 只是 hint：绑定资源会 clamp，直接 provider 也必须在 I/O 前
clamp；调用方未指定 hint 时，provider 自选 page 仍必须遵守该上限。

Capability 表示“当前 configured filesystem 保证支持”，不表示：

- provider 在另一个账号或 region 可能支持；
- 某次操作可能碰巧成功；
- 应用可以跳过运行时错误处理。

### 7.2 Required 不允许静默降级

`AtomicityRequirement`：

- `Required`：成功必须原子；无法保证时必须在副作用前返回
  `RequirementNotMet`；
- `Preferred`：允许 fallback，但成功结果必须报告实际 atomicity；
- `NotRequired`：调用方不要求原子性，provider 仍可选择原子实现。

这个设计遵循一个基本不变量：

```text
请求中的 requirement 决定“什么结果可以被称为成功”；
outcome 描述“这次成功实际上发生了什么”。
```

`ReadOptions`、`WriteOptions`、`DeleteOptions`、`RenameOptions`、`CopyOptions` 与
`PersistOptions` 都提供 `validate_against` preflight。它们把缺失保证映射到明确的
typed capability；provider 必须在 I/O 或副作用前调用相应校验。
此外，`WriteOptions::validate` 会拒绝模型内部不可能同时成立的组合，例如
`CreateNew + IfMatch`，从而避免不同 provider 给出相互矛盾的解释。

### 7.3 Outcome 报告事实

成功结果使用：

- `WriteOutcome`；
- `RenameOutcome`；
- `CopyOutcome`；
- `PersistOutcome`。

它们报告 `AchievedAtomicity`、`PublicationMethod` / `CopyMethod`、统计、版本和
`NonSensitiveMetadata` diagnostics。Diagnostics 必须通过校验 setter 构造，其自动
Debug 只显示 key。调用方不需要根据 provider 名称猜测实际语义。

## 8. Error 模型

`FsError` 是 namespace、open、metadata、provider 与 lifecycle 操作的统一错误，
包含：

- `FsErrorKind`；
- `FsOperation`；
- primary path 与 target path；
- provider id；
- required capability；
- 已清洗的 message 与保留的 source error。

`Debug` 与 `Display` 都不展开 source error；`Debug` 仅通过 `source_present` 报告
是否存在 source，`Display` 完全省略它。显式诊断仍可通过 `Error::source()` 访问。
这样底层 HTTP、SDK 或 I/O 错误中的 signed URL、token 与认证诊断不会意外进入普通
日志。

关键分类包括：

- `UnsupportedOperation`：模型本身不支持；
- `UnsupportedCapability`：缺少稳定 capability；
- `RequirementNotMet`：操作存在，但无法满足强保证；
- `InvalidState`：handle 生命周期不允许；
- `Indeterminate`：副作用可能发生，但最终状态未知；
- `PreconditionFailed`：版本或 existence condition 不满足；
- authentication、permission、timeout、quota、corruption、I/O 等。

字节传输遵循 `qubit-io`，返回 `std::io::Error`；控制面操作返回 `FsError`。
`FsError::into_io_error` 会把完整 `FsError` 保留为 source，
`FsError::from_io` 则在跨回文件系统边界时补充 operation context。

`exists` 只把明确 `NotFound` 转换成 `false`。权限、认证、网络和 timeout 不能被伪装
成“不存在”。

## 9. Writer 生命周期

Writer 不只是 `Output<u8>`，还是 publication session：

```text
                 commit success
Open ─────────────────────────────► Committed
 │
 ├─ abort success ────────────────► Aborted
 │
 ├─ retryable commit failure ─────► Open（session 保留）
 ├─ target not published ─────────► NotPublished（仅可清理）
 ├─ target published ─────────────► Published（仅可清理）
 ├─ uncertain lifecycle failure ──► Indeterminate（session 保留）
 │
 └─ polled async lifecycle future
    dropped before completion ────► Indeterminate（session 保留）
```

`commit(&mut self)` / `commit_async(&mut self)` 不消费 handle：

- 只有 `WriteFailureState::Retryable` 失败后可重试；
- `NotPublished` / `Published` 保留 session 供显式 cleanup，但禁止再次 commit；
- indeterminate 失败后仍可通过 provider session 执行显式 recovery；
- commit/abort 成功后禁止继续写入；
- abort indeterminate session 只表示 staging cleanup 成功，不保证已经发布的 target
  被回滚。

同步 drop 只对确定仍为 Open 的 writer 执行 best-effort abort，错误只能记录；不会
自动处理 `Indeterminate`。异步 drop 不允许启动或阻塞 executor，只对确定仍为 Open
的 writer 调用 provider 的 nonblocking `cancel_on_drop` hook。异步 lifecycle future
从未被 poll 时不改变状态；一旦 poll 后在完成前被 drop，就转为 `Indeterminate`。
需要确认远程清理的调用方必须显式 await `abort_async`。

## 10. 临时资源策略

### 10.1 不提供隐式默认

“临时文件”在不同后端上的真实含义可能是：

- 本地同目录 staging file；
- 系统 temp directory；
- 对象存储随机 key；
- multipart upload；
- 云盘隐藏目录；
- 分布式文件系统同 namespace staging path；
- provider-native upload session。

核心层无法安全推断 namespace、权限、生命周期、成本和 publication atomicity。因此：

- `FileSystem::create_temp_file/dir` 默认返回 `UnsupportedCapability`；
- async 对应方法同样默认不支持；
- provider 必须实现 native strategy，或在 provider config 中要求显式 strategy；
- capability 必须声明 `TempFile`、`TempDirectory` 和
  `AtomicTempPersist` 的保证；
- `TempFileOptions` / `TempDirOptions` 的 `parent: None` 由已配置 provider 解释，
  不是核心层猜测一个路径。

### 10.2 Ownership

`TempFile` / `TempDir` 与 async 对应物拥有 cleanup responsibility：

```text
Owned
 ├─ persist success ──────────────► Persisted
 ├─ keep success ─────────────────► Kept
 ├─ cleanup success ──────────────► Cleaned
 ├─ target published, source left ► CleanupRequired
 └─ final state unknown ──────────► Indeterminate
```

### 10.3 Persist 失败不能丢句柄

`persist` 接收 `&mut self`，失败不会消费临时 handle。错误类型
`PersistFailure` 同时携带 `FsError` 与 `PersistFailureState`：

| Failure state | 已确认事实 | 合法恢复 |
| --- | --- | --- |
| `NotPublished` | target 未发布，source 仍由 handle 拥有 | 修正参数后重试，或 cleanup/keep |
| `PublishedSourceRetained` | target 已发布，source 仍由 handle 拥有 | 禁止 republish；cleanup 或 keep source |
| `Indeterminate` | source/target 最终状态不确定 | 禁止自动 cleanup 与 blind retry；外部 reconciliation |

成功则返回 `PersistOutcome`，明确 target、actual atomicity 与 publication method。

同步 drop 只对 `Owned` / `CleanupRequired` 执行 best-effort cleanup，不自动处理
`Indeterminate`。异步 cleanup、keep、persist future 从未被 poll 时不改变状态；一旦
poll 后在完成前被 drop，handle 就转为 `Indeterminate`，异步 drop 不再 local cancel。
确认 cleanup 需要 await。

### 10.4 Child 安全

`TempDir::child(&FsName)` 只接受单一安全 component；
`descendant(&RelativeFsPath)` 只接受不能逃逸的相对 descendant。绝对 path 无法替换
temporary root，`..` 也无法越过 root。

这只解决 lexical composition。Local provider 仍需处理 symlink、mount point、race
与平台 canonicalization 等真实 filesystem 安全问题。

## 11. Registry 与 Provider SPI

### 11.1 同步 registry

`FileSystemRegistry` 基于 `qubit-spi`：

- provider 实现 `ProviderDefinition<FileSystemSpec>`；
- `FileSystemSpec::Config = FileSystemConfig`；
- `FileSystemSpec::Output = FileSystemResolution<dyn FileSystem>`；
- provider descriptor 携带 id、alias、priority 与 fallback selection 信息；
- registry clone 共享注册状态。

`resource(&FileSystemConfig)` 与 `file_system(&FileSystemConfig)` 是完整配置入口。
`resource_uri(&FsUri)` 与 `file_system_uri(&FsUri)` 只是无 option、无 credential 的
便捷形式。

### 11.2 异步 registry

`AsyncFileSystemProvider` 独立于同步 SPI：

```rust
pub trait AsyncFileSystemProvider: Send + Sync {
    fn descriptor(&self) -> ProviderDescriptor;
    fn create_configured_async<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>>;
}
```

`AsyncFileSystemRegistry` 在 await provider code 前 snapshot candidate，不跨 await
持有 registry lock。一个 provider 可以只实现 async，而不被迫伪造同步 filesystem。
Fallback 成功时直接返回第一个成功结果；若多个候选都失败或 policy 在已有 fallback
attempt 后终止，则最终 `FsError` 的 source 会按尝试顺序保留全部 provider failure，
不会只报告最后一个错误。

### 11.3 Resolution 不只是 filesystem

如果 registry 只返回 filesystem，核心层就不得不重新解释 URI path，从而破坏
provider 语义。`FileSystemResolution` 因此同时返回：

1. configured filesystem；
2. decoded provider-local path；
3. safe canonical URI。

`FileResource` 由这三个结果构造，后续打开的 file handle 继承同一 canonical identity。

## 12. Provider 一致性约束

每个 provider 实现必须遵守以下不变量：

1. `info()`、`capabilities()` 与 `limits()` 不触发 I/O，并在对象生命周期内稳定；
2. capability 只声明当前配置真正保证的行为；
3. `FsUriPath` 只由 provider 按自己的语义解释；hierarchical native path 逐 component
   转换，并拒绝解码后产生的 separator、root 或 prefix；
4. canonical URI 不含 credential；
5. mandatory option 不可静默忽略；
6. `AtomicityRequirement::Required` 无法满足时，在副作用前失败；
7. 成功 outcome 报告实际 method 与 atomicity；
8. file handle 必须携带固定 `OpenedFileInfo`；
9. open-time metadata 只是 snapshot，不冒充 live stat；`stat` 不跟随最终符号链接；
10. write、persist 和 cleanup 失败必须保留可恢复 session；
11. partial success 使用 typed state，不依赖错误字符串；
12. async drop 不执行阻塞或远程 I/O；
13. error 附带 operation、path、target、provider、capability 与 source context；
14. 所有 debug-visible 扩展 metadata 都使用 `NonSensitiveMetadata`；
15. unsupported optional operation 返回明确 capability error；
16. 有限 `max_list_page_entries` 在 provider I/O 前执行，超大 `page_size` hint 被
    clamp。

## 13. 简洁性边界

这套设计通过以下方式保持简洁：

- 应用只面对两套同形 operation trait；
- 公共 property 提升到一个父 trait；
- 文件流统一复用 `qubit-io`；
- concrete type-erased handle 隐藏 provider 泛型；
- resource convenience object 避免反复传 filesystem 与 path；
- option 表示请求，outcome 表示事实，error 表示失败上下文；
- provider-specific 信息进入 `NonSensitiveMetadata`，不污染核心枚举，也不让自动
  Debug 暴露 value；
- 具体 backend、runtime adapter 和 secret resolver 留在组装层。

简洁不等于丢失语义。核心 API 只抽象真正稳定的共同结构，并为差异保留明确扩展点。
