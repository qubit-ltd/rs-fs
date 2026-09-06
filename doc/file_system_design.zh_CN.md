# Qubit FS 文件系统抽象层设计

> 状态：目标设计已与仓库当前 API 与契约对齐。

## 1. 设计目标

`qubit-fs` 为本地、远程、对象存储和其他文件型 provider 提供语义诚实的公共层。
它不把所有 provider 强行伪装成 POSIX 文件系统。

目标如下：

1. 应用只依赖具体、可克隆的 `FileSystem` 或 `AsyncFileSystem` 门面；
2. provider 只实现最小操作 SPI，不重复实现公共校验与资源状态机；
3. capability、limit、requirement、outcome 和 error 明确表达 provider 差异；
4. 强语义无法满足时必须失败，不能静默降级；
5. URI、凭据入口、临时资源、提交和部分成功具有明确安全边界；
6. 同步与异步 API 语义一致；异步取消需要保留恢复状态时允许使用显式 operation
   handle，核心不绑定异步 runtime；
7. provider 可独立发布和注册，不修改核心 crate；
8. 所有公共工具能力都由具体类型的固有方法或关联方法组织，不增加 public free
   function。

非目标包括：

- 承诺每个 provider 支持全部操作；
- 在核心层实现本地平台算法；
- 把 object key 强制解释成目录路径；
- 为 provider 猜测临时 namespace、权限或 publication 策略；
- 让 secret 进入 canonical `Uri`、metadata、错误显示或调试输出；
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
  ├─ reader、writer、directory stream、temp handle
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

## 3. Provider、configured filesystem 与 handle

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

### 3.3 Resolution 与 opened handle

`Path` 只在某个 configured filesystem 的名字空间内有意义，不单独承担跨 filesystem
定位职责。`qubit-fs` 核心不提供 `FileLocation`、`FileResource` 或
`AsyncFileResource`；URI resolution 的绑定结果由 registry 的
`FileSystemResolution` / `AsyncFileSystemResolution` 表达。

Reader、writer、directory stream 和 temporary resource 表示由 configured filesystem
创建的有状态会话。Opened handle 保存固定的 `OpenedFileInfo`、必要时保存 owning
filesystem 的克隆，以及完成生命周期所需的公共契约状态；它们不保存裸 SPI getter。

## 4. URI 与 Path 语义

### 4.1 URI 与 path 的边界

`ConnectionUri` 是 registry/provider 的选择与配置输入；绑定完成后，canonical resource
`Uri` 只表示选定 configured filesystem 上下文内的资源位置。它不包含 filesystem identity，
不能独立解析为跨 provider 位置。`path` 是 provider 解码后的内部资源表示：

```text
ConnectionUri
  └─ registry/provider 选择、凭据提取、校验和解码
       ├─ FileSystem
       ├─ Path
       └─ canonical Uri
```

核心层不能提前执行会破坏 provider 语义的解码或规范化。

### 4.2 `Uri`

`Uri` 是对经过验证的 RFC 3986 URI 实现的薄领域包装。底层解析器负责通用语法，
`qubit-fs` 只负责 filesystem 领域约束，并且不把底层第三方 URI 类型暴露到公共 API。
底层表示必须保留原始词法差异，不采用会重写 path、authority 或 percent-encoding 的
WHATWG URL 规范化。初始实现使用 `fluent_uri::Uri<String>`；该实现细节可以在不破坏
`qubit_fs::Uri` 公共 API 的情况下替换。

`Uri` 保留：

- 小写且经过语法校验的 scheme；
- 可选 authority；
- authority component 是否出现；
- 尚未按 provider 语义解码的 raw URI path；
- ordered、允许 duplicate key 的非敏感 query。

以下表示不可合并：

```text
file:/tmp/a
file:///tmp/a
object://bucket/a%2Fb
```

`Uri` 不解码 `%2F`、不规范化 dot segment、不合并 repeated separator，也不推断
hierarchy。它的普通 `Display` 可以无损输出 canonical、secret-free URI。这里的
secret-free 约束是不可弱化的标准凭据底线；URI 仍然只在对应 filesystem 上下文中解释，
不能因为可持久化就变成独立的全局资源地址。

### 4.3 Credential 边界

URI 输入和长期保存的 canonical URI 使用不同类型：

- `ConnectionUri` 是 registry/configuration ingress，可以接受 authority userinfo
  中的 password，以及 password、token、access key、secret key、signed-URL
  signature 等敏感 query；
- `ConnectionUri` 可以在受控配置解析期间保留原始文本；只有不含敏感 component 时，
  `try_to_uri` 才能生成 `Uri`。`expose_unredacted` 仅向显式 callback 临时提供原文，
  不得用于日志、序列化、metadata、错误消息或 cache key；
- `ConnectionUri` 的 `Display` 和 `Debug` 必须通过 `qubit-redact` 输出脱敏结果；
- 默认解析使用固定的 `RedactionPolicy::standard()`；这是不可移除的安全底线。
  `Uri::parse_with_policy` 始终先应用该标准策略，自定义策略只能增加 provider-specific
  字段规则，不能让标准敏感 component 变得可接受；`ConnectionUri` 保存传入的策略快照，
  用于后续 secret 分类和脱敏格式化；
- 脱敏直接基于 RFC URI component，不得先转换为可能改写原始表示的
  `url::Url`；
- 原始值只能通过名称醒目的显式暴露方法读取，不能依赖 `Display::to_string`；
- `ConnectionUri` 不实现可静默取得明文的 `Deref<Target = str>`、`AsRef<str>` 或
  普通 raw getter；
- `ConnectionUri` 默认不实现普通明文序列化，也不能直接作为 registry cache key；
- registry/provider 在受控解析过程中提取 credential，并产生 secret-free 的
  canonical `Uri`；
- canonical `Uri` 禁止 password 和 credential-like query；provider 只有在确认
  username 属于非敏感地址身份时才可保留 username；
- 两种 URI 都禁止 fragment。

其他 secret 边界保持不变：

- `UserMetadata` 拒绝 credential-like key；
- `NonSensitiveMetadata` 不公开可变 value 入口；
- `FsError` 的 `Display` 和 `Debug` 不自动展开底层 source error。

完整 secret value 只存在于 `ConnectionUri` 和 registry/provider 的受控配置解析过程，
不进入 SPI request、canonical `Uri`、公开 handle 或其他 `qubit-fs` 值对象。标准策略未识别
的 provider 私有凭据仍由 provider 负责消费、移除和私下保存；自定义策略不是转移这项责任的
许可。

### 4.4 `Path`

`Path` 是拥有所有权的、configured filesystem 内的 UTF-8 provider-local 逻辑路径：

- hierarchical semantics 使用 normalized parse；
- object-key 或 provider-specific semantics 使用 literal parse；
- `PathSemantics` 由 `FileSystemProperties` 中的 `FileSystemInfo` 声明。

安全组合使用：

- `PathComponent`：单一非空 component；
- `RelativePath`：非空、normalized、不能是 absolute，且不能逃逸 relative root。

与 `std::path::Path` 同时使用时，adapter 应通过 `LogicalPath`、`NativePath` 等明确
别名区分两套类型。

### 4.5 Native path 转换边界

SPI request 和 outcome 只携带逻辑 `Path`，provider-native path 不越过 adapter 边界。
统一的是转换时机、失败归责和副作用约束，不是所有 provider 的 native path Rust
类型或编码。

平台 native path codec 不属于 `qubit_fs` 核心 facade。它由具体 provider 的 native
文件层拥有；`qubit-fs-local` 通过 `qubit-local-files::LocalPaths` 使用
`LocalPathCodec`，不在 adapter 中复制平台算法，也不把 native path 类型带入
`FileSystemSpi` / `AsyncFileSystemSpi`。对本地文件系统：

- 平台 separator、root、prefix、`OsStr` 和原始字节等转换逻辑位于
  `qubit-local-files`；
- `qubit-fs-local` 只实现逻辑 `Path` 与原生层 API 的薄适配，不复制平台算法；
- URI 或逻辑 path 必须逐 component 转换，并拒绝解码后产生额外 separator、root
  或 prefix 的 component。

每次多路径操作必须先成功转换全部输入路径，再开始任何 provider I/O 或副作用。
Provider 返回的 native path 必须在离开 adapter 前解码为逻辑 `Path`。

## 5. 具体门面

### 5.1 `FileSystemProperties`

原 `FileSystemProperties` trait 改为不可变值类型：

```rust
#[derive(Clone, Debug)]
pub struct FileSystemProperties {
    info: FileSystemInfo,
    capabilities: FileSystemCapabilities,
    limits: FileSystemLimits,
    path_constraints: PathConstraints,
    symlink_policy: SymlinkPolicy,
}

impl FileSystemProperties {
    pub fn new(
        info: FileSystemInfo,
        capabilities: FileSystemCapabilities,
        limits: FileSystemLimits,
        path_constraints: PathConstraints,
        symlink_policy: SymlinkPolicy,
    ) -> FsResult<Self>;
}
```

它是 configured filesystem 的构造时快照：

- getter 不执行本地或远程 I/O；
- 内容在门面生命周期内稳定；
- capability 描述当前配置真正保证的能力；
- unknown、inapplicable、unbounded 和有限 limit 必须明确区分。
- `PathConstraints` 以稳定数据声明 provider-specific path 限制，门面可在 I/O
  前统一执行；平台编码算法不进入该值对象。
- `SymlinkPolicy` 声明 provider 默认的符号链接遍历边界；`ListOptions` 与
  `CopyOptions` 可按操作覆盖。抽象层只承诺拒绝或在当前 filesystem 内跟随，不能把
  host/rooted 的平台策略直接暴露给上层。

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
- `read_all`、`write_all`。

`FileSystemExt` 被删除。

具有类型化部分成功状态的操作不使用普通 `FsResult`：

```rust
pub fn copy(
    &self,
    source: &Path,
    target: &Path,
    options: CopyOptions,
) -> Result<CopyOutcome, CopyFailure>;

pub fn rename(
    &self,
    source: &Path,
    target: &Path,
    options: RenameOptions,
) -> Result<RenameOutcome, RenameFailure>;
```

`FileSystem` 是廉价、显式的共享所有权 handle。`clone()` 只克隆两个 `Arc`；它不能
实现 `Copy`，因为复制 `Arc` 必须更新引用计数。

### 5.3 异步门面

```rust
#[derive(Clone)]
pub struct AsyncFileSystem {
    spi: Arc<dyn spi::AsyncFileSystemSpi>,
    properties: Arc<FileSystemProperties>,
}
```

不需要跨 await 保存恢复责任的异步 I/O 操作使用 inherent `async fn`，方法名与同步
语义相同：

```rust
let metadata = async_file_system.stat(&path).await?;
```

类型名称已经表达异步模式，因此不再给方法附加 `_async`。同步与异步门面共享值类型，
但分别实现 I/O 和生命周期逻辑。Properties getter 和 capability/limit/path
constraint 查询不执行 I/O，仍是普通同步方法。`AsyncFileSystem` 与同步门面一样只
实现 `Clone`，不实现 `Copy`。

异步流式 copy 可能持有需要恢复的 writer，因此不提供会在 future 取消时丢失内部
handle 的普通 `async fn copy`。门面先同步构造一个拥有路径、options、filesystem
和生命周期状态的操作 handle：

```rust
pub fn begin_copy(
    &self,
    source: Path,
    target: Path,
    options: CopyOptions,
) -> Result<AsyncCopyOperation, AsyncCopyFailure>;
```

`begin_copy` 只做同步的 options、path、capability 和静态 limit preflight，不调用
provider。它失败时使用 `CopyFailureState::Unchanged`，且不存在 recovery handle。
调用者通过 `AsyncCopyOperation::execute(&mut self).await` 执行。异步 `rename`
仍返回 `Result<RenameOutcome, RenameFailure>`；它没有门面内部 writer
recovery responsibility。

## 6. Provider SPI

### 6.1 命名空间

Provider API 只通过 `qubit_fs::spi` 暴露：

```text
qubit_fs::spi::FileSystemSpi
qubit_fs::spi::AsyncFileSystemSpi
qubit_fs::spi::StatRequest
qubit_fs::spi::CopyRequest
qubit_fs::spi::CopyAttempt
qubit_fs::spi::SpiCopyFailure
qubit_fs::spi::OpenedWriter
qubit_fs::spi::FileWriterSpi
```

SPI 类型不在 crate 根部或普通 prelude 中重导出。

### 6.2 不可伪造的 request

每个 SPI 操作接收已经完成公共校验的 request：

```rust
pub struct CopyRequest<'a> {
    source: &'a Path,
    target: &'a Path,
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

公开的 `CopyOptions`、`ReadOptions`、`WriteOptions`、`ListOptions`、
`CreateDirectoryOptions`、`DeleteOptions`、`PersistOptions`、`RenameOptions`、
`TempOptions` 和 `FileMetadata` 都是
`#[non_exhaustive]` 值类型。字段不作为跨 crate 的构造或变更接口；调用者使用
`Default`、具名构造方法、`with_*` builder 和只读 getter。这样新增语义只扩展统一的
值类型 API，不要求 provider 或下游重新维护结构体字面量。

Request 不携带 native path。Adapter 收到 request 后必须先转换该操作涉及的全部逻辑
路径，转换失败时不得开始 provider I/O；provider session 可以在内部保存 native
handle 或 native path。

### 6.3 最小 provider 原语

`FileSystemSpi` 包含不可由门面可靠推导的操作：

- `properties`；
- `stat`；
- `list`；
- `open_reader`、`open_writer`；
- `create_directory`；
- `delete_file`、`delete_directory`；
- `rename`；
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

    fn try_copy(
        &self,
        _request: CopyRequest<'_>,
    ) -> Result<CopyAttempt, SpiCopyFailure> {
        Ok(CopyAttempt::Declined(
            CopyDeclineReason::NotImplemented,
        ))
    }

    fn rename(
        &self,
        request: RenameRequest<'_>,
    ) -> Result<RenameOutcome, SpiRenameFailure>;

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
- URI resolution 和 filesystem/path 绑定：由 registry 表达；
- 不改变 provider 语义且可可靠组合的其他 convenience operation。

“最小原语”不等于“操作系统保证原子”。原子性和 durability 仍通过 request requirement
与 outcome 表达。

### 6.4 Copy fast path

`copy` 是门面操作，不是所有 provider 都必须重复实现的 SPI 原语。SPI 只提供
operation-specific、带默认实现的可选 fast path：

```rust
#[non_exhaustive]
pub enum CopyAttempt {
    Completed(CopyOutcome),
    Declined(CopyDeclineReason),
}

#[non_exhaustive]
pub enum CopyDeclineReason {
    NotImplemented,
    NotApplicable,
}
```

暂不抽取通用 `SpiAttempt<T>`。只有出现第二个具有相同拒绝与 fallback 语义的真实操作
时，才将该模式泛化。

`Declined` 的契约是：

- 可以执行 metadata、拓扑或其他只读探测；
- 不能创建、删除或修改 source、target 或其他 namespace entry；
- 不能留下需要调用方 cleanup 的 session、staging entry 或 reservation；
- 不能把已经产生副作用的 native error 映射为 `Declined`；
- `EXDEV` 等错误只有在 adapter 能依据 native 契约证明零副作用时，才可转换为
  `NotApplicable`。

只有 `Declined` 允许门面考虑 fallback。`SpiCopyFailure` 是终止结果，门面不能捕获
其中的 `Unsupported`、I/O error 或其他错误后自动重试。
`NotImplemented` 若与 properties 声明的 provider-native required mode 直接矛盾，
属于 `ProviderContractViolation`；`NotApplicable` 表示经过只读判断后，本次路径对
不满足该 fast path 的动态前提。

`FileSystemCapability::Copy` 表示 provider 声明了这个 native fast path。它不是普通文件
流式 fallback 的必要前提：即使没有 `Copy`，只要 `Read`、`Write` 和 fallback allowlist
满足，门面仍会直接执行流式 copy。`ServerSideCopy`、`AtomicFileCopy`、
`AtomicTreeCopy`、`DurableFileCopy` 与 `DurableTreeCopy` 等更强语义仍然要求
与实际 source mode 匹配的 capability，不能由流式 fallback 推导。

### 6.5 Provider 返回对象

SPI 不直接构造公开 handle，而是返回 `spi` 模块的中间对象：

| SPI 返回值 | 门面包装结果 |
| --- | --- |
| `OpenedReader` | `FileReader` |
| `OpenedWriter` | `FileWriter` |
| `OpenedDirectoryStream` | `DirectoryStream` |
| `OpenedTempFile` | `TempFile` |
| `OpenedTempDirectory` | `TempDirectory` |

中间对象的 provider 构造器公开；转换为公开 handle 的入口仅在 `qubit-fs` 内可见。

`OpenedXxx` 表示一次成功 open 的返回信封，`XxxSpi` 表示具有行为的 provider
session trait，两者不能混用。若某个 `OpenedXxx` 除 `Box<dyn XxxSpi>` 外不携带任何
open-time 信息，应删除该信封并直接返回 session，而不是把信封改名为 `XxxSpi`。

有状态 provider session trait 包括：

- `FileWriterSpi`；
- `DirectoryStreamSpi`；
- `TempResourceSpi`；
- 对应的异步 SPI。

Reader 只需要 type-erased `qubit_io::Input<Item = u8>` 或异步输入以及
`OpenedFileInfo`，不增加没有行为的 reader session trait。

### 6.6 Object safety 与异步

同步与异步 SPI 分离。`AsyncFileSystemSpi` 通过标准 `Future` 的类型擦除别名
`spi::SpiFuture<'a, T>` 保持 object safety 和 runtime neutrality：

```rust
pub type SpiFuture<'a, T> =
    Pin<Box<dyn Future<Output = T> + Send + 'a>>;
```

`SpiFuture` 不是自定义 future 实现。公开 `AsyncFileSystem` 隐藏 boxed future
细节：普通操作通过 inherent `async fn` 返回公共 result，需要保留恢复责任的复合操作
则通过 owning operation handle 执行。异步 `try_copy` 返回
`SpiFuture<'a, Result<CopyAttempt, SpiCopyFailure>>`，与同步 SPI 遵循相同的
declined、failure 和副作用契约。

不使用 public macro 生成两套 API，也不使用一个带同步/异步模式开关的 trait。

## 7. 调用、转换与校验流水线

每个操作遵循固定流程，并明确门面、adapter 与 provider 原生实现各自负责的阶段：

1. 校验公开 options 的内部一致性；
2. 按属性快照中的 `PathSemantics` 校验 source 和 target；
3. 使用属性快照解析默认值；
4. 检查 required capabilities；
5. 检查 path、component、range、write size、page size 等静态 limits；
6. 按属性快照中的 `PathConstraints` 检查 provider 声明的静态路径限制；
7. 门面构造不可伪造、只携带逻辑 `Path` 的 SPI request；
8. adapter 在任何 I/O 前转换本次请求的全部输入路径；
9. provider 原生实现执行操作，或在声明 `Copy` fast path 时由其无副作用地 `Declined`；
10. adapter 将 provider 返回的 native path 解码为逻辑 `Path`；
11. 门面校验 provider 返回的 metadata、entry、opened identity 和 outcome。

步骤 1—7 由门面执行，步骤 8 和 10 由 adapter 执行，步骤 9 由 provider 原生实现
执行。多路径操作在全部输入路径转换成功前不得产生副作用。Directory stream 等惰性
结果可以逐项解码，但每个 entry 都必须在交给门面前完成转换并携带明确错误。

`FileSystem::copy` 在通用步骤之后使用模板方法。声明 `Copy` 时先尝试 SPI fast path；未
声明时直接进入 fallback：

```text
Copy capability?
  ├─ yes ─► spi.try_copy(request)
  │           ├─ Completed ──► 门面复核 CopyOutcome
  │           ├─ Declined ──► 检查 fallback 矩阵并流式复制
  │           └─ Failure ───► 转换为 CopyFailure，不 fallback
  └─ no ──► 检查 fallback 矩阵并流式复制
```

门面 fallback 重新调用公开 reader/writer 所对应的私有门面流程；每个组合原语仍执行
自己的 path conversion、capability、limit、lifecycle 和 outcome 校验。门面不能直接
绕过这些流程调用 reader/writer SPI。

Provider 仍负责只能在执行时发现的条件：

- 权限、认证与远程可用性；
- 当前资源状态和 precondition；
- 动态配额、剩余空间；
- mount、跨设备或其他运行时边界；
- 无法通过稳定 capability/limit 静态表达的 provider 条件。

门面校验 provider 返回值至少包括：

- opened filesystem identity 和逻辑路径与请求一致；
- directory entry 属于请求 namespace，并满足 prefix/filter contract；
- required atomicity 没有被降级；
- copy/rename/write/persist outcome 与实际请求相容；
- provider 返回路径符合声明的 path semantics；
- metadata 和 capability 组合不自相矛盾。

## 8. 位置边界与公开 handle

### 8.1 位置身份

`Path` 只标识 configured filesystem 内的逻辑路径；它不包含 filesystem identity，也
不能独立解析为跨 provider 位置。

canonical `Uri` 也只在对应的 configured filesystem 上下文内表示资源位置。它可以作为
registry 的持久化或选择输入，但不携带 filesystem identity，不能脱离 resolution 独立解析为
全局资源地址。

`qubit-fs` 不公开 `FileLocation`、`FileResource` 或 `AsyncFileResource`：

- registry 使用自己的 `FileSystemResolution` /
  `AsyncFileSystemResolution` 表达 `FileSystem + Path + canonical Uri`；
- opened handle 通过 `OpenedFileInfo` 保存必要的 `FileSystemId + Path` snapshot；
- writer 和 temporary resource 在生命周期确有需要时私有保存 owning
  `FileSystem` / `AsyncFileSystem`；
- 不为绑定路径复制整套 `FileSystem` convenience API。

若未来需要类似 Spring `Resource` 的 `FileResource`、`UrlResource` 等统一资源模型，
应在独立的 `qubit-resource` crate 中设计，并由它适配 `qubit-fs`。

### 8.2 Reader

`FileReader` / `AsyncFileReader` 保存固定 `OpenedFileInfo` 与 type-erased input。
Provider 返回的 filesystem identity 和逻辑 `Path` 在门面包装前验证。Open-time
metadata 只是 snapshot，不能冒充 live `stat`。

底层字节流继续使用：

- `qubit_io::Input<Item = u8>`；
- `qubit_io::Output<Item = u8>`；
- `qubit_io::AsyncInput<Item = u8>`；
- `qubit_io::AsyncOutput<Item = u8>`。

这样同步与异步组合能力保持一致，异步核心也不绑定 runtime。任意 `Input<u8>` 不等于
文件 reader；只有门面把 `spi::OpenedReader` 与经过校验的 `OpenedFileInfo` 组合后，
才能产生 `FileReader`。Public handle 的直接 `new` 构造器删除。

`read_all` 在分配前检查已知长度，并在流式读取时再次执行实际字节上限，防止 provider
metadata 缺失或错误导致无界分配。若 `ReadOptions` 指定 `offset` 或 `length`，预判使用
请求窗口的长度，而不是完整资源长度：

```text
selected = min(max(resource_length - offset, 0), requested_length)
```

未指定 `length` 时，窗口延伸到资源末尾；`FileMetadata::len` 始终表示完整资源长度。
因此一个 1 MiB 资源的 8 字节 range 在 `max_bytes = 8` 时可以通过预判。实际流读取仍
逐批执行预算检查，metadata 缺失时也不能绕过上限。`read_all` 仍然先打开 reader，以
保留 NotFound、权限和条件版本错误；本次不把 `read_prefix` 自动转换成 range read。

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

### 9.1 Capability 与动态适用性

`FileSystemCapabilities` 是稳定 capability set，`FileSystemLimits` 是独立快照。
Capability 表示当前 configured filesystem 在明确静态前提下稳定支持的语义范围，
不是任意 source、target、mount、device、bucket 或资源状态下都必然成功的承诺。
Provider 必须能对超出动态适用范围的请求在副作用前明确拒绝。

Capability 不作为 fast-path 分派开关：

- capability 用于副作用前排除必然无法满足的 requirement；
- `CopyAttempt::Declined` 表示本次 request 的 native fast path 动态不适用；
- requirement 表示本次调用不可降级的条件；
- outcome 表示实际完成方式。

`FileSystem::from_spi` 根据 provider 声明的原语能力和核心可证明的 fallback 前提，生成
对应用可见的 effective capability snapshot。例如，只有 reader、create-new writer
及其必要生命周期保证同时存在时，门面才可把基础流式 file copy 计入 `Copy`
capability。`ServerSideCopy` 等 capability 只描述语义支持范围；门面仍调用
`try_copy` 判断具体路径对是否适用。

基础 `FileSystem` 不承诺完整 quota、POSIX permission、Windows ACL 或对象存储
ACL/IAM 管理 API。这些模型的作用域和语义不同，不能放入所有 provider 都必须面对的
基础契约。核心仍保留 `PermissionDenied`、`QuotaExceeded` 等操作错误，并可暴露真正
可移植的只读 metadata。

出现明确下游需求后，quota 或 access control 应分别设计为强类型的可选 capability
门面和 SPI，而不是向基础 `FileSystem` 堆叠方法，也不使用 `Any` extension bag。
本地平台特有的 mode、owner 和 ACL 算法留在 `qubit-local-files`。

### 9.2 Copy fallback 边界

第一版通用 fallback 只覆盖能够在 provider-neutral 原语上证明安全的普通文件子集：

- source 已验证为普通 file/object，不处理 directory tree、symlink 或特殊 entry；
- 不启用 `continue_on_error`；
- metadata preservation 为 `None`；
- 不要求 server-side copy、clone/reflink 或 provider-native method；
- source 与 target 的逻辑 `Path` 不同；
- target 使用 create-new/no-replace publication；
- `Skip` 通过 create-new 的 already-exists 结果实现；
- 不执行通用 `Overwrite` fallback；
- 不隐式创建 parent 或执行其他未被 failure state 覆盖的前置副作用。

`CopyMode::Tree` 永远不能进入单文件 fallback。`CopyOptions` 带有与 filesystem 默认
`SymlinkPolicy` 不同的 override 时也必须在 `stat`、`open_reader` 或 `open_writer` 前
拒绝；override 缺省或等于默认策略时才可继续 allowlist 检查。这样 fallback 不会把
普通文件操作伪装成树复制，也不会静默忽略符号链接策略。原生 `try_copy` 成功路径可以
支持 Tree 或策略 override；这些限制只约束无原生实现或原生明确 `Declined` 后的 fallback。

Fallback 需要 `Read` 与 `Write`，但不需要 `Copy`；`Copy` 缺失只表示 provider-native
fast path 不可用。要求 server-side、atomicity 或 durability 的 request 仍在副作用前
根据对应 capability 失败。

门面按 allowlist 判断 resolved options；不在列表中的组合在打开 target writer 前返回
`RequirementNotMet`、`UnsupportedCapability` 或对应的 options error。以后只有在
staged writer、可靠 replacement、alias 安全和失败恢复都能证明时，才扩展通用
overwrite fallback。

流式 fallback：

1. 在分配或写入前检查已知 source length 与 read/write limits；
2. 打开经过门面包装的 reader；
3. 以 create-new disposition 打开经过门面包装的 writer；
4. 有界流式传输 bytes；
5. commit writer 并复核 write outcome；
6. 返回 `CopyMethod::Streamed` 的 `CopyOutcome`。

任何不能保证的 atomicity、durability、metadata 或 publication requirement 都必须在
副作用前失败。

### 9.3 Copy outcome 与 failure

`CopyMethod` 报告实际执行方式，而不是笼统的 provider 类型：

```rust
#[non_exhaustive]
pub enum CopyMethod {
    Streamed,
    Native,
    Clone,
    ServerSide,
    Mixed,
}
```

`Clone` 覆盖 reflink、APFS clone 等 copy-on-write 方法。`CopyOutcome` 至少报告：

- actual method；
- actual atomicity 与 durability；
- metadata preservation 结果；
- bytes、files、directories、skipped 等统计；
- target version（provider 可提供时）；
- 是否使用 fallback；
- 非敏感 diagnostics。

`FsResult<CopyOutcome>` 无法表达 target 已部分可见或完整发布后的错误，因此公共
`copy` 使用类型化 failure：

```rust
pub enum CopyFailureState {
    Unchanged,
    PartiallyPublished,
    Published,
    Indeterminate,
}

pub struct CopyFailure {
    error: FsError,
    state: CopyFailureState,
    partial_stats: CopyStats,
    writer: Option<FileWriter>,
}
```

状态含义：

| `CopyFailureState` | 已确认事实 |
| --- | --- |
| `Unchanged` | target 未被本次操作改变 |
| `PartiallyPublished` | target 已创建或修改，但请求的 copy 尚未完整完成 |
| `Published` | target 内容已完整发布，后续 metadata 或 durability 步骤失败 |
| `Indeterminate` | 无法确认 target 的最终状态 |

同步门面只有在 fallback 仍把 recovery responsibility 交给调用者时，才在
`CopyFailure` 中返回 writer。异步版本由 `AsyncCopyOperation` 保留可恢复的
`AsyncFileWriter`；`AsyncCopyFailure` 只报告 error、typed state 和 partial stats，
operation 是 recovery handle 是否存在的唯一事实来源。Provider-native
`SpiCopyFailure` 携带相同的 typed state 和 partial stats，但不构造公共 writer。

Copy 的 source 必须保持不变。Provider 修改 source 属于
`ProviderContractViolation`。`try_copy` 返回 failure 后，门面直接转换并返回，不能
依据错误种类再次执行流式复制。

### 9.4 Rename 与 move

`rename` 是真正的 namespace primitive，继续由 SPI 必须实现，不能用 copy+delete
模拟。`RenameOutcome` 报告实际 publication method 与 atomicity。当前 durability
只由 `CopyOutcome` 建模；write、rename 和临时资源 persist 不得宣称 durable
publication。`SpiRenameFailure` / `RenameFailure` 使用 `Unchanged`、`Renamed`、
`Indeterminate` 等类型化状态表达 rename 的已确认进度。
`FileSystemCapability::Rename` 只表示 rename，不再使用“rename or move”的含混定义。

```rust
pub enum RenameFailureState {
    Unchanged,
    Renamed,
    Indeterminate,
}
```

`RenameFailure` 包含统一 `FsError` 和 `RenameFailureState`；异步版本只更换类型名称，
不改变状态语义。

当前不增加 `move`、`try_move` 或 `MoveOutcome`。如果未来出现明确的 copy+delete
需求，它必须作为新的高层操作设计，并在调用者允许非原子退化时才启用；“target 已
发布但 source 删除失败”必须是独立的类型化部分成功状态。

Requirement 决定什么结果可以称为成功；outcome 描述实际发生的事实。例如：

- `AtomicityRequirement::Required`：无法保证时在副作用前失败；
- `Preferred`：允许 fallback，但 outcome 必须报告实际 atomicity；
- `NotRequired`：调用者不要求，provider 仍可采用更强实现。

成功结果使用 `WriteOutcome`、`RenameOutcome`、`CopyOutcome` 和 `PersistOutcome`，
报告各自模型所覆盖的 actual atomicity、copy durability、publication/copy method、版本、
统计和非敏感 diagnostics。

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

会产生类型化部分成功状态的操作使用 operation-specific failure 包装 `FsError`：

- `CopyFailure` / `AsyncCopyFailure`；
- `RenameFailure`；
- 已定义的 `WriteAllFailure`、writer failure 和 persist failure。

门面 preflight failure 统一映射为对应操作的 `Unchanged` / `NotStarted` 状态。
Operation-specific failure 不复制 provider identity、path 或 source error，而是复用
内部 `FsError` 的统一上下文。

新增：

```rust
FsErrorKind::ProviderContractViolation
```

它表示 provider bug 或不合法返回，例如 required-atomic 请求却报告非原子成功、list
返回越界 entry、opened identity 不匹配或 outcome 自相矛盾。

`RequirementNotMet` 表示运行时条件使一个合法请求无法满足；它不是 provider contract
violation。

`exists` 只把明确 `NotFound` 转换为 `false`。权限、认证、timeout 或 I/O 错误不能被
伪装为不存在。

## 11. Writer 与 `write_all`

### 11.1 Writer 状态

```text
Open
 ├─ commit success ───────────────► Committed
 ├─ abort / NotPublished ─────────► Aborted
 ├─ abort / Published ────────────► Published
 ├─ abort / Indeterminate ────────► Indeterminate
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

`WriteAbortOutcome` 独立报告 cleanup 完成后的 namespace 事实：

- `NotPublished`；
- `Published`；
- `Indeterminate`。

abort 的成功只表示 cleanup 已完成，不能被解释为 destination 一定未发布。

`FileWriter` 保存 open 时解析的 write contract。`commit(&mut self)` 负责：

- 调用 `FileWriterSpi::commit`；
- 复核 `WriteOutcome`；
- 更新公开状态；
- 在 provider 报告成功但违反 required semantics 时返回
  `ProviderContractViolation`，并按已知事实进入 `Published`，不能伪装成未写入。

非 `Open` writer 再次调用 `commit` 仍返回 `InvalidState`，但 failure 中的
`WriteFailureState` 必须反映 writer 已知的发布事实：`Committed` 映射为 `Published`，
`Aborted`/`NotPublished` 映射为 `NotPublished`，`Published` 保持 `Published`，
`Indeterminate` 保持 `Indeterminate`。这次非法调用不得再次调用 provider commit、
不得自动 abort，也不得改变 writer 状态；“本次调用非法”不能覆盖历史上已经确认的发布事实。

`abort` 可用于清理 `Open`、`NotPublished`、`Published` 或 `Indeterminate` session。
对 `Published` 的 abort 只能清理 staging，绝不能回滚已经发布的 target。
facade 必须以 `WriteAbortOutcome` 更新公开状态，不能从 `Ok` 自行推断
`Aborted`。一次 abort 成功后 cleanup 即为终态；即使公开状态仍是 `Published` 或
`Indeterminate`，重复 abort 也返回 `InvalidState`。abort 失败则保留 session，允许
调用者重试；确定性失败恢复 abort 前状态，未知结果进入 `Indeterminate`。

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

`TempFile` / `TempDirectory` 私有保存创建它的 `FileSystem` 克隆和逻辑 `Path`，
因此始终绑定同一个 configured filesystem 实例。异步 handle 对应保存
`AsyncFileSystem`。

门面在包装 public handle 前复核 SPI 返回的 temp identity/kind。若 opened temp
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
| `PublishedSourceRetained` | `CleanupRequired` | target 已发布，source 仍需清理 |
| `Indeterminate` | `Indeterminate` | source 或 target 最终状态未知 |

成功返回 `PersistOutcome` 时 handle 进入 `Persisted`。失败仅使用以上三种
`PersistFailureState`，不会以 `Published` 作为额外失败状态。

同步 drop：

- `Owned`、`CleanupRequired`：best-effort cleanup；
- `Persisted`、`Kept`、`Cleaned`：不操作；
- `Indeterminate`：不自动修改文件系统。

确认清理必须显式调用 `cleanup`。

### 12.3 Child 安全

`TempDirectory::child(&PathComponent)` 和 `AsyncTempDirectory::child` 只接受单一安全
component；`descendant(&RelativePath)` 只接受不能逃逸的相对路径，同步和异步 handle
保持相同的路径构造语义。Provider 仍负责 symlink、mount point、race 和平台
canonicalization 等真实边界。

## 13. 异步生命周期

异步 drop 不启动 executor、不阻塞线程，也不暗中发起远程 I/O。

### 13.1 Handle method future

异步 commit、abort、persist、keep 和 cleanup future：

- 尚未 poll 即 drop，不改变 handle 状态；
- poll 后未完成即 drop，进入 `Indeterminate`；
- 需要确认恢复或清理时必须显式 await；
- provider 可提供明确 nonblocking 且无远程副作用的 drop hook，但它不能替代确认语义。

同步与异步状态枚举、failure state 和 outcome 保持一致。

不持有 recovery session 的 one-shot namespace future（例如 rename）在开始 poll 后被
取消时，调用者必须把已知路径状态视为 indeterminate 并重新检查。若某个操作可能把
需要继续驱动或 cleanup 的 session 隐藏在 future 内部，它必须改用显式 operation
handle，不能继续暴露为消费内部状态的普通 `async fn`。

### 13.2 `AsyncCopyOperation`

`AsyncCopyOperation` 是异步 copy 的 owning lifecycle handle：

```rust
pub enum AsyncCopyOperationState {
    Ready,
    Running,
    Completed,
    Failed(CopyFailureState),
}

pub struct AsyncCopyOperation {
    file_system: AsyncFileSystem,
    source: Path,
    target: Path,
    options: ResolvedCopyOptions,
    state: AsyncCopyOperationState,
    writer: Option<AsyncFileWriter>,
}
```

字段全部私有。它提供：

- `source()`、`target()` 和 `state()` 只读 getter；
- `execute(&mut self) -> Result<CopyOutcome, AsyncCopyFailure>` 执行 provider fast
  path 或门面 fallback；
- `has_recovery_writer()` 报告 operation 当前是否仍拥有 recovery writer；
- `recovery_writer()` 在 operation 仍持有 writer 时返回可变借用；
- `take_recovery_writer()` 显式转移 writer cleanup/recovery responsibility。

状态转换如下：

```text
Ready
  └─ execute polled ───────────────► Running
       ├─ success ─────────────────► Completed
       ├─ explicit failure ────────► Failed(reported state)
       └─ future cancelled ────────► Failed(Indeterminate)
```

`execute` future 借用 `&mut AsyncCopyOperation`，不能消费 operation。开始 poll 后，
取消 guard 必须在 future drop 时把仍为 `Running` 的状态改为
`Failed(Indeterminate)`。如果门面 fallback 已创建 writer，writer 继续保存在
operation 中；调用者可显式恢复、abort 或取走它。

如果取消发生在 provider-native `try_copy` 内且 provider 没有返回可恢复 session，
operation 仍保存 filesystem、source 和 target 供调用者检查，但 recovery writer 为
`None`。此时不能猜测 target 是否改变。

`execute` 只接受 `Ready` 状态；`Completed`、`Running` 或 `Failed` 状态不能盲目重试。
调用者检查失败事实并完成必要 cleanup 后，可以显式创建新的 operation。

`AsyncCopyOperation` 的 drop 不执行 I/O。Drop 一个 `Ready` 或 `Completed`
operation 不产生动作；Drop 一个 `Failed` operation 会显式放弃仍由它持有的 recovery
session，但不会暗中执行 cleanup。调用者必须在 drop 前通过 `state()` 和
`has_recovery_writer()` 判断恢复责任，并按需使用或取走 writer。

### 13.3 Copy deadline

`CopyOptions::deadline` 是从 copy operation 构造时开始计算的累计、协作式时间预算。
同步 `copy` 在 operation 构造时开始计时；异步 `begin_copy` 在返回 operation 时开始计时，
即使尚未调用 `execute`，等待时间也计入预算。它不承诺硬中断；provider 一次不可中断的
调用必须先返回，门面在下一检查点报告超时。

检查点覆盖原生调用前及其成功或 `Declined` 后、fallback 各阶段成功后和下一次 I/O 前、
每轮 read/write 前后、`flush` 前后以及 `commit` 前后。provider 已返回的错误优先保留，
不能被 deadline 错误覆盖。超时发生在 writer 已打开但尚未发布时，failure 保留 writer
及其状态；`flush` 超时不得继续 `commit`。若 `commit` 已成功而之后超时，target 已发布，
failure 必须是 `Published`，保留成功统计且不返回可重试的已完成 writer。

## 14. Registry 集成边界

`qubit-fs-registry` 负责：

- provider discovery 和 selection；
- 完整配置与 credential reference；
- 消费 `ConnectionUri` 并在受控边界提取 credential；
- URI provider-specific 解码；
- canonical URI 去敏；
- 返回具体 `FileSystemResolution` 或 `AsyncFileSystemResolution`。

Resolution 保存门面、provider-local `Path` 和 canonical `Uri`，不保存
`ConnectionUri`、credential 或 `Arc<dyn FileSystemSpi>`。Canonical `Uri` 必须在
resolution 返回前通过不可弱化的标准 secret-free 结构校验；provider 私有凭据必须由 provider
在进入该边界前消费或移除。

构造 resolution 时必须复用核心的无 I/O 路径校验：`Path` 的 semantics、form、component
以及 provider 声明的 path limits 都要与 configured filesystem 的
`FileSystemProperties` 一致。校验失败时 resolution 构造失败；成功只表示静态绑定有效，
不代表执行 `stat` 或其他 provider I/O 已成功。

`qubit-fs` 不依赖 registry，也不重新解释 provider 已解码的 URI path。

## 15. 模块与公开 API

目标模块：

```text
src/
├── file_system.rs
├── async_file_system.rs
├── file_system_properties.rs
├── spi/
│   ├── *_file_system_spi.rs
│   ├── *_request.rs
│   ├── opened_*.rs
│   └── *_session.rs
├── copy/
├── rename/
├── handle/
├── temp/
├── options/
├── metadata/
├── path/
├── uri/
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
| `FileSystemSpi::copy` | `FileSystemSpi::try_copy` + 门面 fallback |
| `FsUri` | `Uri` + `ConnectionUri` |
| `FsPath` | `Path` |
| `RelativeFsPath` | `RelativePath` |
| `FsName` | `PathComponent` |
| `spi::FsFuture` | `spi::SpiFuture` |
| `FileLocation` / `FileResource` | 删除；registry 使用 resolution |

Crate 根部只重导出应用层类型；SPI 始终保留在 `spi` 命名空间。

## 16. Provider 必须遵守的不变量

1. `properties()` 不执行 I/O，并返回构造时稳定值；
2. capability 只声明当前配置在明确前提下稳定支持的语义范围；
3. provider 只接收门面构造、携带逻辑 `Path` 的 request；
4. adapter 在任何 I/O 前转换一次操作的全部输入路径；
5. adapter 在返回门面前解码所有 provider-native path；
6. mandatory option 不可忽略；
7. required semantics 无法满足时不得返回成功；
8. `CopyAttempt::Declined` 不得产生外部可见副作用或遗留 cleanup responsibility；
9. `try_copy` 返回 failure 后门面不得自动 fallback；
10. copy 不得修改 source，failure 必须报告 target typed state 与 partial stats；
11. rename 不得以 copy+delete 模拟；
12. outcome 报告实际 method、fallback、atomicity、durability、版本和 side effect；
13. opened handle identity 与请求 filesystem 和逻辑路径一致；
14. list entry 不能离开请求 namespace；
15. copy、rename、write、persist 和 cleanup 的部分成功使用 typed state；
16. `AsyncCopyOperation::execute` 借用 operation，取消后不得丢失门面 fallback writer；
17. async copy 取消后必须进入 `Failed(Indeterminate)`，不能伪造 `Unchanged`；
18. async drop 不执行阻塞或远程 I/O；
19. `ConnectionUri`、error source 和 diagnostics 不通过普通格式化泄漏 secret；
20. provider-specific native 算法留在 provider 原生实现，不复制到 adapter；
21. SPI session 的 cleanup 不能擅自回滚已确认 published target；
22. provider contract violation 不能降格为普通 I/O 错误。
23. registry resolution 必须校验 path semantics、form 和 limits；成功不代表已执行 I/O。
24. range read 的 max-bytes 预判按选定窗口计算，不能用完整资源长度误拒绝合法窗口。
25. 非 Open writer 的重复 commit 保留已知 publication state，且不得再次触发 provider I/O。
26. deadline 超时不能覆盖已经返回的 provider error；发布后超时必须报告 Published。

## 17. 验证策略

`qubit-fs` 自身使用 recording 和 fault-injecting SPI 验证：

- 所有确定性校验发生在 SPI 调用前；
- 外部代码无法构造 SPI request；
- 属性快照只采集一次并保持稳定；
- opened object 和 outcome 会被门面复核；
- `try_copy` 的 `Completed` 路径不执行 fallback；
- `Declined` 只有在 fallback allowlist 满足时才执行流式复制；
- `SpiCopyFailure`、任意 I/O error 和 unsupported error 都不会触发重试；
- recording SPI 验证 `Declined` 前后没有 namespace mutation 或 cleanup responsibility；
- 普通文件 create-new/skip fallback、overwrite 拒绝和所有 requirement preflight；
- `CopyFailureState`、partial stats 及需要恢复时保留的 writer；
- `AsyncCopyOperation` 的 Ready/Running/Completed/Failed 全部状态转换；
- 分别在 native attempt、reader、writer 和 commit await 点取消 copy future；
- 取消后 operation 保留 recovery writer，drop 不触发 I/O；
- rename 始终调用 SPI 原语，不进入 copy+delete；
- writer、directory stream、temp 的全部状态转换；
- `write_all` 在失败时保留恢复 handle；
- `ConnectionUri` 的脱敏输出、canonical `Uri` 的 secret-free 约束和 error context；
- 同步/异步 requirement、failure state 和 outcome 语义对齐；
- async future cancellation 和 drop 语义。

Provider 的黑盒一致性由 `qubit-fs-testkit` 通过公开门面验证。Provider 自己仍需测试
平台、协议、编码、安全和性能边界；adapter 测试必须覆盖多路径转换失败无副作用、
native path 解码、provider-specific `PathConstraints`、copy decline 零副作用、
native copy method 映射和 rename identity 语义。
