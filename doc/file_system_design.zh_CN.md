# Qubit FS 文件系统抽象层设计

> 状态：已批准的目标设计。本文定义重构完成后的长期架构与 provider 契约；
> 在重构完成前，仓库中的现有 API 可能与本文不同。

## 1. 设计目标

`qubit-fs` 为本地、远程、对象存储和其他文件型 provider 提供语义诚实的公共层。
它不把所有 provider 强行伪装成 POSIX 文件系统。

目标如下：

1. 应用只依赖具体、可克隆的 `FileSystem` 或 `AsyncFileSystem` 门面；
2. provider 只实现最小操作 SPI，不重复实现公共校验与资源状态机；
3. capability、limit、requirement、outcome 和 error 明确表达 provider 差异；
4. 强语义无法满足时必须失败，不能静默降级；
5. URI、凭据、临时资源、提交和部分成功具有明确安全边界；
6. 同步与异步 API 形态一致，但核心不绑定异步 runtime；
7. provider 可独立发布和注册，不修改核心 crate；
8. 所有公共工具能力都由具体类型的固有方法或关联方法组织，不增加 public free
   function。

非目标包括：

- 承诺每个 provider 支持全部操作；
- 在核心层实现本地平台算法；
- 把 object key 强制解释成目录路径；
- 为 provider 猜测临时 namespace、权限或 publication 策略；
- 在 URI、metadata、错误显示或调试输出中携带 secret；
- 让应用直接调用 provider SPI；
- 为同步和异步 API 构造一个带模式开关的统一 trait；
- 绑定 Tokio、`futures-io` 或某个 executor。

## 2. 分层与依赖方向

```text
应用
  │
  ▼
qubit-fs
  ├─ FileSystem / AsyncFileSystem 具体门面
  ├─ resource、reader、writer、directory stream、temp handle
  ├─ path、URI、metadata、options、outcomes、errors
  └─ qubit_fs::spi provider 扩展点
       ▲
       │ 实现 SPI
provider adapter（例如 qubit-fs-local）
       │
       ▼
provider 原生实现（例如 qubit-local-files）

qubit-fs-registry ── 发现并创建 provider，返回 FileSystem 门面
qubit-fs-testkit  ── 通过公开门面验证 provider 契约
```

依赖规则：

- `qubit-fs` 不依赖 registry 或具体 provider；
- provider adapter 依赖 `qubit-fs` 及其原生实现；
- 原生实现不依赖 `qubit-fs`；
- registry 依赖 `qubit-fs` 和 `qubit-spi`，但不取得 filesystem operation SPI；
- testkit 只通过公开门面测试 provider，不绕过门面构造 SPI request。

## 3. Provider、configured filesystem 与 resource

### 3.1 Provider

Provider 是 registry 层的配置和 URI 适配工厂。它负责：

- 声明 provider id、alias 和优先级；
- 校验完整配置；
- 解析 credential reference；
- 解释 URI authority、path 与 query；
- 构造 configured filesystem；
- 返回 provider-local path 与安全 canonical URI。

Provider 本身不是文件系统操作对象。`FileSystemProvider` 这一名称保留给 registry
工厂，因此 operation 扩展接口命名为 `FileSystemSpi`，不使用含义冲突的
`FileSystemProvider`。

### 3.2 Configured filesystem

`FileSystem` 或 `AsyncFileSystem` 表示一次配置完成后的文件系统。例如，同一个
provider 可以因 endpoint、region、bucket、root 或 credential profile 不同而产生多个
filesystem。

`FileSystemInfo::id()` 标识 configured filesystem；
`FileSystemInfo::provider_id()` 标识创建它的 provider。两者不可混用。

### 3.3 Resource 与 opened handle

`FileResource` / `AsyncFileResource` 把一个 provider-local `FsPath` 绑定到 owning
filesystem。Reader、writer、directory stream 和 temporary resource 表示由这个
filesystem 创建的有状态会话。

Resource 保存门面，不保存裸 SPI；opened handle 保存固定的 `OpenedFileInfo` 和完成
生命周期所需的公共契约状态。

## 4. URI 与 Path 语义

### 4.1 URI 与 path 的边界

URI 是跨边界定位及 provider 选择表示；path 是 provider 解码后的内部资源表示：

```text
FsUri
  └─ registry/provider 选择、校验和解码
       ├─ FileSystem
       ├─ FsPath
       └─ canonical FsUri
```

核心层不能提前执行会破坏 provider 语义的解码或规范化。

### 4.2 `FsUri`

`FsUri` 保留：

- 小写且经过语法校验的 `FsScheme`；
- 可选 `FsAuthority`；
- authority component 是否出现；
- 尚未按 provider 语义解码的 `FsUriPath`；
- decoded、ordered、允许 duplicate key 的非敏感 `FsUriQuery`。

以下表示不可合并：

```text
file:/tmp/a
file:///tmp/a
object://bucket/a%2Fb
```

`FsUriPath` 不解码 `%2F`、不规范化 dot segment、不合并 repeated separator，也不
推断 hierarchy。

### 4.3 Credential 边界

URI 和所有可自动显示的 metadata 必须 secret-free：

- authority 禁止 password；
- query 禁止 password、token、access key、secret key、signed-URL signature 等
  credential-like key；
- URI fragment 禁止；
- `UserMetadata` 拒绝 credential-like key；
- `NonSensitiveMetadata` 不公开可变 value 入口；
- `FsError` 的 `Display` 和 `Debug` 不自动展开底层 source error。

完整 secret value 只存在于 registry/provider 的受控配置解析过程，不进入
`qubit-fs` 值对象。

### 4.4 `FsPath`

`FsPath` 是 configured filesystem 内的 UTF-8 provider-local path：

- hierarchical semantics 使用 normalized parse；
- object-key 或 provider-specific semantics 使用 literal parse；
- `PathSemantics` 由 `FileSystemProperties` 中的 `FileSystemInfo` 声明。

安全组合使用：

- `FsName`：单一非空 component；
- `RelativeFsPath`：非空、normalized、不能是 absolute，且不能逃逸 relative root。

Provider 把 URI 或 `FsPath` 转为 native path 时，必须逐 component 转换，并拒绝解码
后产生平台 separator、root 或 prefix 的 component。

## 5. 具体门面

### 5.1 `FileSystemProperties`

原 `FileSystemProperties` trait 改为不可变值类型：

```rust
#[derive(Clone, Debug)]
pub struct FileSystemProperties {
    info: FileSystemInfo,
    capabilities: FileSystemCapabilities,
    limits: FileSystemLimits,
}

impl FileSystemProperties {
    pub fn new(
        info: FileSystemInfo,
        capabilities: FileSystemCapabilities,
        limits: FileSystemLimits,
    ) -> FsResult<Self>;
}
```

它是 configured filesystem 的构造时快照：

- getter 不执行本地或远程 I/O；
- 内容在门面生命周期内稳定；
- capability 描述当前配置真正保证的能力；
- unknown、inapplicable、unbounded 和有限 limit 必须明确区分。

字段私有，provider 通过 `FileSystemProperties::new` 构造。该方法检查属性内部一致性；
`FileSystem::from_spi` 在 SPI trust boundary 再次执行防御性校验。

### 5.2 同步门面

```rust
#[derive(Clone)]
pub struct FileSystem {
    spi: Arc<dyn spi::FileSystemSpi>,
    properties: Arc<FileSystemProperties>,
}
```

构造入口：

```rust
impl FileSystem {
    pub fn from_spi<S>(spi: S) -> FsResult<Self>
    where
        S: spi::FileSystemSpi + 'static;

    pub fn from_shared_spi(
        spi: Arc<dyn spi::FileSystemSpi>,
    ) -> FsResult<Self>;
}
```

构造时读取、校验并缓存属性。门面不公开 `spi()`、`into_spi()` 或任何可绕过公共契约
的入口。

所有应用操作都是 inherent methods，包括：

- `stat`、`exists`、`list`；
- `open_reader`、`open_writer`；
- `create_directory`、`delete_file`、`delete_directory`；
- `copy`、`rename`；
- `create_temp_file`、`create_temp_directory`；
- `resource`；
- `read_all`、`write_all`。

`FileSystemExt` 被删除。

### 5.3 异步门面

```rust
#[derive(Clone)]
pub struct AsyncFileSystem {
    spi: Arc<dyn spi::AsyncFileSystemSpi>,
    properties: Arc<FileSystemProperties>,
}
```

异步门面的 I/O 操作使用 inherent `async fn`，方法名与同步语义相同：

```rust
let metadata = async_file_system.stat(&path).await?;
```

类型名称已经表达异步模式，因此不再给方法附加 `_async`。同步与异步门面共享值类型，
但分别实现 I/O 和生命周期逻辑。Properties getter、capability/limit 查询和
`resource`/`resource_at` 等纯绑定操作不执行 I/O，仍是普通同步方法。

## 6. Provider SPI

### 6.1 命名空间

Provider API 只通过 `qubit_fs::spi` 暴露：

```text
qubit_fs::spi::FileSystemSpi
qubit_fs::spi::AsyncFileSystemSpi
qubit_fs::spi::StatRequest
qubit_fs::spi::CopyRequest
qubit_fs::spi::OpenedWriter
qubit_fs::spi::FileWriterSpi
```

SPI 类型不在 crate 根部或普通 prelude 中重导出。

### 6.2 不可伪造的 request

每个 SPI 操作接收已经完成公共校验的 request：

```rust
pub struct CopyRequest<'a> {
    source: &'a FsPath,
    target: &'a FsPath,
    options: &'a ResolvedCopyOptions,
}
```

实际字段全部私有，构造器为 `pub(crate)`，只提供只读 getter。外部 provider 可以实现
SPI 并读取 request，但普通调用者不能在 safe Rust 中构造 request。

公开 options 表达调用者意图；`Resolved*Options` 表达门面已经：

- 校验内部组合；
- 解析默认值；
- 应用 path semantics；
- 检查 capabilities；
- 检查并应用 limits。

Provider 不重复这些通用检查。

### 6.3 最小 provider 原语

`FileSystemSpi` 包含不可由门面可靠推导的操作：

- `properties`；
- `stat`；
- `list`；
- `open_reader`、`open_writer`；
- `create_directory`；
- `delete_file`、`delete_directory`；
- `copy`、`rename`；
- `create_temp_file`、`create_temp_directory`。

同步 SPI 的结构如下：

```rust
pub trait FileSystemSpi: Send + Sync {
    fn properties(&self) -> FileSystemProperties;

    fn stat(
        &self,
        request: StatRequest<'_>,
    ) -> FsResult<FileMetadata>;

    fn list(
        &self,
        request: ListRequest<'_>,
    ) -> FsResult<OpenedDirectoryStream>;

    fn open_reader(
        &self,
        request: OpenReaderRequest<'_>,
    ) -> FsResult<OpenedReader>;

    fn open_writer(
        &self,
        request: OpenWriterRequest<'_>,
    ) -> FsResult<OpenedWriter>;

    fn create_directory(
        &self,
        request: CreateDirectoryRequest<'_>,
    ) -> FsResult<CreateDirectoryOutcome>;

    fn delete_file(
        &self,
        request: DeleteFileRequest<'_>,
    ) -> FsResult<DeleteOutcome>;

    fn delete_directory(
        &self,
        request: DeleteDirectoryRequest<'_>,
    ) -> FsResult<DeleteOutcome>;

    fn copy(
        &self,
        request: CopyRequest<'_>,
    ) -> FsResult<CopyOutcome>;

    fn rename(
        &self,
        request: RenameRequest<'_>,
    ) -> FsResult<RenameOutcome>;

    fn create_temp_file(
        &self,
        request: CreateTempFileRequest<'_>,
    ) -> FsResult<OpenedTempFile>;

    fn create_temp_directory(
        &self,
        request: CreateTempDirectoryRequest<'_>,
    ) -> FsResult<OpenedTempDirectory>;
}
```

`CreateDirectoryOutcome` 明确区分实际创建与 options 允许的 already-existed no-op，
并可报告递归创建的 ancestor 数量。`DeleteOutcome` 明确区分实际删除与 options 允许的
not-found no-op，并可报告递归删除的 entry 数量。未知计数使用 `Option<u64>`，不伪造
精确值；不使用含义未定义的布尔返回值。

以下操作不进入 SPI：

- `exists`：由 `stat` 的明确 `NotFound` 推导；
- `read_all`、`write_all`：由公开 handle 组合；
- `resource`：由门面绑定；
- 不改变 provider 语义且可可靠组合的其他 convenience operation。

“最小原语”不等于“操作系统保证原子”。原子性和 durability 仍通过 request requirement
与 outcome 表达。

### 6.4 Provider 返回对象

SPI 不直接构造公开 handle，而是返回 `spi` 模块的中间对象：

| SPI 返回值 | 门面包装结果 |
| --- | --- |
| `OpenedReader` | `FileReader` |
| `OpenedWriter` | `FileWriter` |
| `OpenedDirectoryStream` | `DirectoryStream` |
| `OpenedTempFile` | `TempFile` |
| `OpenedTempDirectory` | `TempDirectory` |

中间对象的 provider 构造器公开；转换为公开 handle 的入口仅在 `qubit-fs` 内可见。

有状态 provider session trait 包括：

- `FileWriterSpi`；
- `DirectoryStreamSpi`；
- `TempResourceSpi`；
- 对应的异步 SPI。

Reader 只需要 type-erased `qubit_io::Input<Item = u8>` 或异步输入以及
`OpenedFileInfo`，不增加没有行为的 reader session trait。

### 6.5 Object safety 与异步

同步与异步 SPI 分离。`AsyncFileSystemSpi` 通过 `spi::FsFuture<'a, T>` 保持 object
safety 和 runtime neutrality。公开 `AsyncFileSystem` 隐藏 boxed future 细节。

不使用 public macro 生成两套 API，也不使用一个带同步/异步模式开关的 trait。

## 7. 门面校验流水线

每个操作在任何 provider I/O 或副作用前执行固定流程：

1. 校验公开 options 的内部一致性；
2. 按属性快照中的 `PathSemantics` 校验 source 和 target；
3. 使用属性快照解析默认值；
4. 检查 required capabilities；
5. 检查 path、component、range、write size、page size 等静态 limits；
6. 构造不可伪造的 SPI request；
7. 调用 SPI；
8. 校验 provider 返回的 metadata、entry、location 和 outcome。

Provider 仍负责只能在执行时发现的条件：

- 权限、认证与远程可用性；
- 当前资源状态和 precondition；
- 动态配额、剩余空间；
- mount、跨设备或其他运行时边界；
- 无法通过稳定 capability/limit 静态表达的 provider 条件。

门面校验 provider 返回值至少包括：

- opened location 与请求路径一致；
- directory entry 属于请求 namespace，并满足 prefix/filter contract；
- required atomicity 没有被降级；
- copy/rename/write/persist outcome 与实际请求相容；
- provider 返回路径符合声明的 path semantics；
- metadata 和 capability 组合不自相矛盾。

## 8. Resource 与公开 handle

### 8.1 `FileResource`

```rust
#[derive(Clone)]
pub struct FileResource {
    file_system: FileSystem,
    location: FileLocation,
}
```

门面提供两个绑定入口：

```rust
impl FileSystem {
    pub fn resource(
        &self,
        path: FsPath,
    ) -> FsResult<FileResource>;

    pub fn resource_at(
        &self,
        location: FileLocation,
    ) -> FsResult<FileResource>;
}
```

`resource` 绑定没有 canonical URI 的普通 provider-local path。`resource_at` 供 registry
等已经完成 URI resolution 的调用者使用，并验证 filesystem id、provider identity、
path semantics、limits 和 canonical URI 的非敏感结构。URI 到 decoded path 的
provider-specific 对应关系仍由创建 `FileLocation` 的 registry/provider 保证。

规则：

- 普通资源通过 `FileSystem::resource` 创建；
- registry resolution 通过 `FileSystem::resource_at` 创建；
- `FileResource::new` 和绕过门面的构造器删除；
- `file_system()` 可返回安全的 `&FileSystem`；
- 不提供 SPI getter；
- resource 方法只委托门面，不重复参数、capability 或 limit 校验。

`AsyncFileResource` 保存 `AsyncFileSystem`，规则相同。

### 8.2 Reader

`FileReader` / `AsyncFileReader` 保存固定 `OpenedFileInfo` 与 type-erased input。
Provider 返回的 opened location 在门面包装前验证。Open-time metadata 只是 snapshot，
不能冒充 live `stat`。

底层字节流继续使用：

- `qubit_io::Input<Item = u8>`；
- `qubit_io::Output<Item = u8>`；
- `qubit_io::AsyncInput<Item = u8>`；
- `qubit_io::AsyncOutput<Item = u8>`。

这样同步与异步组合能力保持一致，异步核心也不绑定 runtime。任意 `Input<u8>` 不等于
文件 reader；只有门面把 `spi::OpenedReader` 与经过校验的 `OpenedFileInfo` 组合后，
才能产生 `FileReader`。Public handle 的直接 `new` 构造器删除。

`read_all` 在分配前检查已知长度，并在流式读取时再次执行实际字节上限，防止 provider
metadata 缺失或错误导致无界分配。

### 8.3 Directory stream

`list` 的 SPI 结果是 `OpenedDirectoryStream`，不是公开 page 或 provider continuation
token。Public `DirectoryStream` 逐项：

- 调用 `DirectoryStreamSpi`；
- 校验 entry path、name、root、prefix 与 metadata；
- 补齐错误上下文；
- 对外提供 inherent collection/convenience methods。

Stream 具有 `Open`、`Exhausted`、`Failed` 状态。Provider error 或 contract violation
使其进入终止 `Failed`，不能继续消费不可信数据。`DirectoryStreamExt` 及异步版本删除。

## 9. Capability、Requirement 与 Outcome

`FileSystemCapabilities` 是稳定 capability set，`FileSystemLimits` 是独立快照。
Capability 表示当前 configured filesystem 保证支持，不表示某次操作可能碰巧成功。

Requirement 决定什么结果可以称为成功；outcome 描述实际发生的事实。例如：

- `AtomicityRequirement::Required`：无法保证时在副作用前失败；
- `Preferred`：允许 fallback，但 outcome 必须报告实际 atomicity；
- `NotRequired`：调用者不要求，provider 仍可采用更强实现。

成功结果使用 `WriteOutcome`、`RenameOutcome`、`CopyOutcome` 和 `PersistOutcome`，
报告 actual atomicity、publication/copy method、版本、统计和非敏感 diagnostics。

门面根据 request 复核 outcome。Provider 不能通过返回“成功”绕过 required semantics。

## 10. Error 模型与归责

`FsError` 保留：

- `FsErrorKind`；
- 面向调用者的 `FsOperation`；
- primary path 和 target path；
- provider identity；
- required capability；
- 已清洗 message；
- 不自动显示的 source error。

门面统一处理上下文：

- provider identity 总是取自属性快照；
- operation 总是对应公开调用；
- SPI 未提供时补齐 source/target path 和 required capability；
- SPI 可以报告更精确的失败子路径；
- source error 被保留，但不能通过普通格式化泄漏 secret。

新增：

```rust
FsErrorKind::ProviderContractViolation
```

它表示 provider bug 或不合法返回，例如 required-atomic 请求却报告非原子成功、list
返回越界 entry、opened location 不匹配或 outcome 自相矛盾。

`RequirementNotMet` 表示运行时条件使一个合法请求无法满足；它不是 provider contract
violation。

`exists` 只把明确 `NotFound` 转换为 `false`。权限、认证、timeout 或 I/O 错误不能被
伪装为不存在。

## 11. Writer 与 `write_all`

### 11.1 Writer 状态

```text
Open
 ├─ commit success ───────────────► Committed
 ├─ abort success ────────────────► Aborted
 ├─ retryable/not published ──────► Open
 ├─ not published/not retryable ──► NotPublished
 ├─ published ────────────────────► Published
 └─ unknown ──────────────────────► Indeterminate
```

`WriteFailureState` 为：

- `RetryableNotPublished`；
- `NotPublished`；
- `Published`；
- `Indeterminate`。

`FileWriter` 保存 open 时解析的 write contract。`commit(&mut self)` 负责：

- 调用 `FileWriterSpi::commit`；
- 复核 `WriteOutcome`；
- 更新公开状态；
- 在 provider 报告成功但违反 required semantics 时返回
  `ProviderContractViolation`，并按已知事实进入 `Published`，不能伪装成未写入。

`abort` 可用于清理 `Open`、`NotPublished`、`Published` 或 `Indeterminate` session。
对 `Published` 的 abort 只能清理 staging，绝不能回滚已经发布的 target。
`Open`/`NotPublished` abort 成功后进入 `Aborted`；`Published`/`Indeterminate`
即使 session cleanup 成功也保留原事实状态，不能用 `Aborted` 抹去 namespace 事实。

同步 drop 只在确定安全的状态下 best-effort abort；`Indeterminate` 不自动操作。

### 11.2 `write_all`

`write_all` 不能只返回 `FsResult<WriteOutcome>`，否则内部 writer 失败时会丢失恢复
session：

```rust
pub struct WriteAllFailure {
    error: FsError,
    writer: Option<FileWriter>,
}
```

- 仍需 retry、abort 或人工核查时返回 writer；
- 已确认完成清理时 writer 为 `None`；
- 调用者可以根据 writer state 选择恢复动作；
- convenience API 不通过 drop 掩盖 publication 或 cleanup 的不确定性。

异步版本携带 `AsyncFileWriter`，遵循相同语义。

## 12. Temporary resource

### 12.1 策略边界

核心层不猜测 provider 的临时目录、namespace、权限、成本或 publication 策略。
Provider 必须通过 capability 和 SPI 明确提供 temp file、temp directory 与 atomic
persist 能力。

`TempFile` / `TempDirectory` 保存由原始 `FileSystem` 构造的 `FileResource`，因此
始终绑定同一个 configured filesystem 实例。

门面在包装 public handle 前复核 SPI 返回的 temp location/kind。若 opened temp
违反契约，门面必须显式调用 session cleanup；cleanup 失败作为
`ProviderContractViolation` 的 secondary/source context 保留，不能把尚未绑定的
cleanup responsibility 静默交给 drop。

### 12.2 状态

```rust
pub enum TempResourceState {
    Owned,
    Persisted,
    Kept,
    Cleaned,
    CleanupRequired,
    Indeterminate,
}
```

`persist(&mut self, target, options)` 失败不消费 handle：

| `PersistFailureState` | Handle 状态 | 已确认事实 |
| --- | --- | --- |
| `NotPublished` | `Owned` | target 未发布，可修正后重试 |
| `Published` | `Persisted` | target 已发布，source ownership 已处理 |
| `PublishedSourceRetained` | `CleanupRequired` | target 已发布，source 仍需清理 |
| `Indeterminate` | `Indeterminate` | source 或 target 最终状态未知 |

`Published` 覆盖 rename 已完成但后续目录 durability 操作失败等情况。方法虽然返回错误，
handle 也不能回到 `Owned`。

同步 drop：

- `Owned`、`CleanupRequired`：best-effort cleanup；
- `Persisted`、`Kept`、`Cleaned`：不操作；
- `Indeterminate`：不自动修改文件系统。

确认清理必须显式调用 `cleanup`。

### 12.3 Child 安全

`TempDirectory::child(&FsName)` 只接受单一安全 component；
`descendant(&RelativeFsPath)` 只接受不能逃逸的相对路径。Provider 仍负责 symlink、
mount point、race 和平台 canonicalization 等真实边界。

## 13. 异步生命周期

异步 drop 不启动 executor、不阻塞线程，也不暗中发起远程 I/O。

异步 commit、abort、persist、keep 和 cleanup future：

- 尚未 poll 即 drop，不改变 handle 状态；
- poll 后未完成即 drop，进入 `Indeterminate`；
- 需要确认恢复或清理时必须显式 await；
- provider 可提供明确 nonblocking 且无远程副作用的 drop hook，但它不能替代确认语义。

同步与异步状态枚举、failure state 和 outcome 保持一致。

## 14. Registry 集成边界

`qubit-fs-registry` 负责：

- provider discovery 和 selection；
- 完整配置与 credential reference；
- URI provider-specific 解码；
- canonical URI 去敏；
- 返回具体 `FileSystemResolution` 或 `AsyncFileSystemResolution`。

Resolution 保存门面、provider-local `FsPath` 和 canonical `FsUri`，不保存或暴露
`Arc<dyn FileSystemSpi>`。Registry resource 必须通过门面的安全 location 绑定入口
构造。

`qubit-fs` 不依赖 registry，也不重新解释 provider 已解码的 URI path。

## 15. 模块与公开 API

目标模块：

```text
src/
├── file_system.rs
├── async_file_system.rs
├── properties.rs
├── spi/
│   ├── file_system_spi.rs
│   ├── async_file_system_spi.rs
│   ├── request/
│   ├── opened/
│   └── session/
├── resource/
├── reader/
├── writer/
├── directory/
├── temp/
├── options/
├── metadata/
├── path/
└── error/
```

主要替换关系：

| 旧 API | 目标 API |
| --- | --- |
| `trait FileSystem` | `struct FileSystem` + `spi::FileSystemSpi` |
| `trait AsyncFileSystem` | `struct AsyncFileSystem` + `spi::AsyncFileSystemSpi` |
| `trait FileSystemProperties` | `struct FileSystemProperties` |
| `Arc<dyn FileSystem>` | `FileSystem` |
| `FileSystemExt` | `FileSystem` inherent methods |
| `DirectoryStreamExt` | `DirectoryStream` inherent methods |
| `FileWriteSession` | `spi::FileWriterSpi` |
| `DirectoryStreamSession` | `spi::DirectoryStreamSpi` |
| `TempResourceSession` | `spi::TempResourceSpi` |

Crate 根部只重导出应用层类型；SPI 始终保留在 `spi` 命名空间。

## 16. Provider 必须遵守的不变量

1. `properties()` 不执行 I/O，并返回构造时稳定值；
2. capability 只声明当前配置真正保证的行为；
3. provider 只接收门面构造的 request；
4. mandatory option 不可忽略；
5. required semantics 无法满足时不得返回成功；
6. outcome 报告实际 method、atomicity、版本和 side effect；
7. opened handle location 与请求一致；
8. list entry 不能离开请求 namespace；
9. write、persist 和 cleanup 的部分成功使用 typed state；
10. async drop 不执行阻塞或远程 I/O；
11. error source 和 diagnostics 不泄漏 secret；
12. provider-specific native 算法留在 provider 原生实现，不复制到 adapter；
13. SPI session 的 cleanup 不能擅自回滚已确认 published target；
14. provider contract violation 不能降格为普通 I/O 错误。

## 17. 验证策略

`qubit-fs` 自身使用 recording 和 fault-injecting SPI 验证：

- 所有确定性校验发生在 SPI 调用前；
- 外部代码无法构造 SPI request；
- 属性快照只采集一次并保持稳定；
- opened object 和 outcome 会被门面复核；
- writer、directory stream、temp 的全部状态转换；
- `write_all` 在失败时保留恢复 handle；
- error context 与 secret boundary；
- 同步/异步行为对称；
- async future cancellation 和 drop 语义。

Provider 的黑盒一致性由 `qubit-fs-testkit` 通过公开门面验证。Provider 自己仍需测试
平台、协议、编码、安全和性能边界。
