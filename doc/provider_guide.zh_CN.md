# Provider 实现指南

[English version](provider_guide.md)

本指南面向 `qubit-fs` 0.3 的 provider 作者，说明如何实现一个能够被
`FileSystem` 门面信任的最小 provider，以及发布适配器前应完成的检查。它不是后端
教程、凭据管理器，也不承诺所有后端都支持每个操作。

## 1. 门面与 SPI 的信任边界

应用调用 `FileSystem`（或 `AsyncFileSystem`），provider 在 `qubit_fs::spi` 下实现
`FileSystemSpi`。`ProviderProperties` 是 provider 的声明；门面会校验声明，并向应用
暴露校验后的 `FileSystemProperties` 有效视图。请让声明保持不变，在 `properties()`
中返回缓存值的副本。

## 2. 缓存 `ProviderProperties`

在构造 provider 时创建并校验一次 `ProviderProperties`。不要每次调用都重新构造，也
不要用 `expect` 把校验推迟到运行时。下面的 `provider-minimal` fixture 展示了这种
所有权方式。

## 3. 只声明已经实现的能力

`ProviderOperations` 列出 provider 实际实现的操作，`FileSystemCapabilities` 描述
更强的语义保证。不要因为后端有相似的底层原语，就顺手声明一个操作或 capability。
从最小声明开始，每增加一项就补一条契约测试。

## 4. capability 依赖的保证强度

Capability 有 `Conditional` 和 `Guaranteed` 两种支持强度。`Conditional` 派生能力
要求基础能力被支持；`Guaranteed` 派生能力要求基础能力也是 `Guaranteed`。基础能力缺失并非唯一的不一致情况：
若派生能力为 `Guaranteed`，基础能力仅为 `Conditional` 也不满足要求。`missing_dependency()` 会报告首个缺失或
强度不足的基础能力，门面会在 provider 使用前拒绝不一致的声明。

## 5. 一个完整的最小 provider

下面的 provider 暴露一个只读健康资源。这段完整代码由文档 fixture 编译验证。它只实现已声明的
顺序 `OpenReader` operation；随机读取仍是可选能力，SPI 默认方法返回 `UnsupportedOperation`。

<!-- example: provider-minimal -->
```rust
use std::io::Cursor;

use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::read::ReadOptions;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

/// A minimal synchronous provider that serves a single health resource.
///
/// The provider reports `/health` as a two-byte file containing `ok` and
/// rejects every other path.
///
/// # Examples
///
/// ```
/// use qubit_fs::FileSystem;
/// use qubit_fs::FsResult;
/// use qubit_fs::Path;
/// use qubit_fs::read::ReadOptions;
/// use qubit_fs_documentation_examples::provider_minimal::HealthProvider;
///
/// fn main() -> FsResult<()> {
///     let filesystem = FileSystem::from_spi(HealthProvider::new()?)?;
///     let body = filesystem.read_all(&Path::parse("/health")?, ReadOptions::default(), 16)?;
///     assert_eq!(body, b"ok");
///     Ok(())
/// }
/// ```
pub struct HealthProvider {
    /// Properties advertised to the filesystem when this provider is mounted.
    properties: ProviderProperties,
}

impl HealthProvider {
    /// Creates a provider configured to serve `/health`.
    ///
    /// # Returns
    ///
    /// A provider with the required stat and reader operations enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the filesystem identity or provider properties are
    /// invalid.
    pub fn new() -> FsResult<Self> {
        let id = FileSystemId::new("docs-health")?;
        let properties = ProviderProperties::new(
            FileSystemInfo::new(id, "docs-health", PathSemantics::Hierarchical),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )?;
        Ok(Self { properties })
    }

    /// Rejects paths other than the single resource served by this provider.
    ///
    /// # Parameters
    ///
    /// - `path`: The requested logical path.
    /// - `operation`: The provider operation being attempted.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` for `/health`.
    ///
    /// # Errors
    ///
    /// Returns a `NotFound` error for any path other than `/health`, preserving
    /// the rejected path and requested operation in the error context.
    fn validate_path(path: &Path, operation: FsOperation) -> FsResult<()> {
        if path.as_str() == "/health" {
            Ok(())
        } else {
            Err(FsError::new(FsErrorKind::NotFound, operation, "health resource not found").with_path(path.clone()))
        }
    }
}

impl FileSystemSpi for HealthProvider {
    /// Returns a clone of the provider properties used when it was created.
    ///
    /// # Returns
    ///
    /// The immutable provider property snapshot.
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }

    /// Reports `/health` as a two-byte regular file.
    ///
    /// # Parameters
    ///
    /// - `request`: The validated request containing the path to inspect.
    ///
    /// # Returns
    ///
    /// Metadata identifying `/health` as a two-byte regular file.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` when the request targets a path other than `/health`.
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Self::validate_path(request.path(), FsOperation::Stat)?;
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(2)),
        ))
    }

    /// Opens `/health` as a reader over the bytes `ok`.
    ///
    /// # Parameters
    ///
    /// - `request`: The validated request containing the path to open.
    ///
    /// # Returns
    ///
    /// A reader bound to this provider's filesystem identity and `/health`.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` when the request targets a path other than `/health`.
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Self::validate_path(request.path(), FsOperation::OpenReader)?;
        Ok(OpenedReader::new(
            OpenedFileInfo::new(self.properties.info().id().clone(), request.path().clone()),
            Box::new(Cursor::new(b"ok".to_vec())),
        ))
    }
}

/// Mounts the minimal provider and reads its `/health` resource.
///
/// # Returns
///
/// Returns the resource bytes, which are `ok` for this provider.
///
/// # Errors
///
/// Returns an error if provider construction, filesystem mounting, path
/// parsing, or reading the resource fails.
pub fn read_health() -> FsResult<Vec<u8>> {
    let filesystem = FileSystem::from_spi(HealthProvider::new()?)?;
    filesystem.read_all(&Path::parse("/health")?, ReadOptions::default(), 16)
}
```

### 最小异步 provider

这个只提供 metadata 的示例展示与运行时无关的 `AsyncFileSystemSpi` 边界。它只声明 `Stat`；返回的 boxed future 在被轮询前不执行 provider 工作。它没有实现可选的随机读取 operation。文档 fixture 会编译并执行这段完整代码。

<!-- example: provider-minimal-async -->
```rust
use qubit_fs::AsyncFileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

/// Provides metadata for one health resource without selecting a runtime.
///
/// # Examples
///
/// ```
/// use qubit_fs::AsyncFileSystem;
/// use qubit_fs::FsResult;
/// use qubit_fs::Path;
/// use qubit_fs_documentation_examples::provider_minimal_async::AsyncHealthProvider;
///
/// fn main() -> FsResult<()> {
///     let future = async {
///         let filesystem = AsyncFileSystem::from_spi(AsyncHealthProvider::new()?)?;
///         let metadata = filesystem.stat(&Path::parse("/health")?).await?;
///         assert_eq!(metadata.len(), Some(2));
///         Ok::<(), qubit_fs::FsError>(())
///     };
///     drop(future);
///     Ok(())
/// }
/// ```
pub struct AsyncHealthProvider {
    /// Immutable declaration returned at the SPI boundary.
    properties: ProviderProperties,
}

impl AsyncHealthProvider {
    /// Creates a provider that advertises only the implemented stat operation.
    ///
    /// # Errors
    /// Returns an invalid-properties error if the static declaration is
    /// invalid.
    pub fn new() -> FsResult<Self> {
        let properties = ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("docs-async-health")?,
                "docs-async-health",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new().with(ProviderOperation::Stat),
            FileSystemCapabilities::new(),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )?;
        Ok(Self { properties })
    }
}

impl AsyncFileSystemSpi for AsyncHealthProvider {
    /// Returns the cached declaration without provider I/O.
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }

    /// Returns metadata when polled, or a contextual not-found error.
    fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async move {
            let path = request.path();
            if path.as_str() != "/health" {
                return Err(
                    FsError::new(FsErrorKind::NotFound, FsOperation::Stat, "health resource not found")
                        .with_path(path.clone()),
                );
            }
            Ok(StatResponse::new(
                path.clone(),
                FileMetadata::new(FileKind::File).with_len(Some(2)),
            ))
        })
    }
}

/// Queries the health resource through the public asynchronous facade.
///
/// # Errors
/// Returns a path, provider, or facade-contract error if metadata is
/// unavailable.
pub async fn stat_health() -> FsResult<FileMetadata> {
    let filesystem = AsyncFileSystem::from_spi(AsyncHealthProvider::new()?)?;
    filesystem.stat(&Path::parse("/health")?).await
}
```

## 6. 错误与上下文

返回 `FsError` 时带上失败的操作和路径。门面会保留操作的 `FsEffectState`（例如发布
是否可能已经对外可见），让调用方能够基于事实选择恢复策略，而不是猜测。只要知道
目标路径或临时路径，就应将其放进错误上下文。

同步读取中，已经传输的字节应先作为成功的短读返回，再报告中断。字节已生效或 effect
不确定时，不要返回可重试的 `Interrupted`；应使用带有相应 effect 状态的类型化 `FsError`，
使 reader 停止聚合重试。`FileReader` 只在外层 I/O 错误直接包含 `FsError` 时提供此保护；
它不会递归检查任意包装错误。

## 7. 发布与清理

writer 和临时资源实现必须分别报告发布、清理和恢复失败。清理失败并不表示目标一定
不存在。遵循 `CopyOutcome` 与写入恢复契约，保留主失败以及清理错误，便于观测和处置。

非空写入已接受前缀、后续写入却没有进展时，发布状态为不确定。Provider 可能已直接暴露已接受的字节；即使 Provider 内部使用暂存，facade 也无法据此推断隔离成立。请保留恢复对象，并依据显式 abort 或核对结果处理；不要盲目重试 commit。

临时资源持久化必须分别报告本次调用的发布事实和源资格：目标明确未发布、但源的变更权限
不确定时使用 `NotPublishedSourceIndeterminate`；目标已发布、但源权限不确定时使用
`PublishedSourceIndeterminate`；目标未发布且只允许清理残留资源时使用
`NotPublishedSourceCleanupRequired`。不得把这些状态折叠成 `Indeterminate`，不得根据
目标校验结果推导源所有权，也不得在重试被拒绝时覆盖先前已确认的发布目标。

## 8. 异步取消

异步操作必须在取消域之外持有操作状态，并一直保留 operation，直到完成清理和恢复
决策。不要让取消丢弃 writer、临时路径或发布结果。取消导致源权限不确定时，仍须保留
此前已确认的发布目标。门面的源生命周期和失败状态可以保守地变为整体
`Indeterminate`；provider 正常返回且能够确定两个维度时，才使用精确的源不确定状态。

异步目录列举有单独的取消契约：丢弃已经 poll 的
`AsyncDirectoryStream::next_entry_async()` future 会使该流进入 `Failed`，因为提供者可能
已经前进。后续读取返回 `InvalidState`，不会再次调用提供者。SPI 不承诺回退或续传；调用方
可以重新打开流，再根据应用保存的进度去重或对账。重新列举没有快照保证，也不保证顺序稳定或
结果不重复。该契约只适用于异步流；同步的 `DirectoryStream::next_entry()` 是同步调用，没有
丢弃 future 的取消边界。

## 9. 接入 `qubit-fs-testkit`

适配器应运行 `qubit-fs-testkit` 的共享契约套件，并针对限制、capability、路径规则
和恢复补充后端测试。发布前运行 fixture 和 all-features 测试。

## 10. 常见契约违规

测试失败时，先检查 operation 与 capability 是否只声明了已实现的内容、capability
依赖强度是否正确、stat 元数据是否与操作结果一致，以及每个错误是否包含路径和操作
上下文。只有 `stat` 明确返回 `NotFound` 时，`exists` 才会返回 false；权限、认证、
超时和 I/O 错误必须继续作为错误返回。

## 11. 列举范围与有界读取

列举层级目录或平面键前缀时，传入 `ListScope::Path(path)`；列举整个已配置的平面
命名空间时，传入 `ListScope::Namespace`。层级文件系统拒绝 Namespace，列举其根目录
应使用 `ListScope::Path(Path::root())`。`Path` 仍拒绝空字符串。Namespace 不会扩大
配置的文件系统边界，也不能用来打开、查询属性或写入资源。

平面键的 `LiteralPrefix` 相对于所选范围匹配。例如根为 `folder/`、过滤器为 `a`
时匹配 `folder/a` 和 `folder/ab`；根为 `folder` 时还会匹配 `folderish`。
匹配过程不补分隔符，也不规范化键文本。Namespace 的过滤器匹配完整逻辑键。
打开流之前，会按 provider 的路径文本上限检查根与过滤器合并后的长度。

列举 deadline 从目录流构造完成时开始计算，每次调用 provider 前后都会检查。
到期后收到的成功条目或 EOF 会被拒绝；实际 provider 错误保留原类型和错误链。
这是一种协作式预算，不能中断永久 Pending 的 future。只构造再丢弃未经 poll 的
next-entry future，不会改变流状态。

`read_prefix` 只打开一次 reader，不额外 stat，消费字节数不超过前缀上限。
只有 `RangeRead` 为 **Guaranteed**、未请求 checksum、前缀长度为正，且范围可表示
并符合 provider 上限时，才会自动添加或收紧 range；原始选项总是先校验。
Conditional 或不支持范围读取的 provider 仍可顺序读取前缀。BestEffort checksum
保留原请求；Required checksum 会返回 `RequirementNotMet`，因为仅读取前缀不能确认
完整校验。需要该保证时使用完整的 `read_all`。返回和消费上限不等于网络预取量保证。

## 12. 0.3 的打开失败与恢复协议

同步、异步门面的 `open_writer`、`create_temp_file` 和 `create_temp_directory`
均返回 `OpenFailure<R>`。`Preflight` 和 `ProviderOpen` 阶段没有可交回的会话；
`OutcomeValidation` 表示 provider 已返回会话，但身份校验失败。此时错误持有
`RejectedWriter`、`RejectedAsyncWriter`、`RejectedTempResource` 或
`RejectedAsyncTempResource`，仅允许显式 abort/cleanup，不提供写入、commit、keep、
persist 或原始 session。应用应保留错误，或用 `take_recovery` 接管会话；库不提供会
丢失恢复责任的普通 `FsError` 转换。

清理失败或已轮询的清理 future 被取消后，会话仍保留，清理状态变为
`RecoveryCleanupState::Indeterminate`；丢弃未轮询的 future 不改变状态。
确认清理完成后状态为 `Completed`，再次清理只返回 `InvalidState`，不调用 provider。
任意成功的 abort 都确认清理完成；`Indeterminate` 只表示目标发布状态未知。隔离句柄的 Drop 不启动清理，也不调用
`cancel_on_drop`。清理必须依据 session 实际拥有的资源，不能依据未校验的诊断路径。
公开清理错误只使用已配置的 provider 和已知请求路径；没有 parent 的临时请求不伪造路径。
主失败和清理错误应同时保留。

设置 `TempOptions::parent()` 时，应在该请求位置下返回临时资源：层级路径按组件判断严格后代，
ObjectKey 按原始文本前缀判断。provider 可以把相对层级 parent 投影为绝对逻辑路径，但必须保留完全相同的
组件前缀；绝对 parent 要求返回绝对路径。门面会在校验 filesystem 身份和资源 kind 后检查位置。
ProviderSpecific 语义和未指定 parent 的请求不推断通用位置关系。词法校验不能替代 provider
对文件系统 authority 和符号链接的保护。

直接 writer 句柄还会分别暴露“本会话曾确认发布”和当前清理状态。这是门面保留的历史事实，
provider 无需新增 SPI 方法或 abort outcome。`WriteAbortOutcome` 仍只表示本次清理尝试的结果。
后续清理返回 `NotPublished` 不能撤销此前确认的发布；清理成功也不能证明已发布目标被回滚。

整文件写入和复制通过 `WriterRecovery` / `AsyncWriterRecovery` 交回资源：
`Opened` 是已验证 writer，`Rejected` 只有清理权限。用 `recovery`、适用时的
`recovery_mut` 及 `take_recovery` 替代旧的 writer 专用访问器。
同步 `WriteAllFailure::state()` 和 `written_bytes()` 保存失败时的事实，包括短写确认
字节数和确切提交状态；abort 或取走会话都不改写历史。`into_parts` 返回错误、状态、
已确认字节数和恢复会话。对于打开阶段的 copy 碰撞，只有 provider 打开失败、明确证明无副作用且没有隔离会话
时，才能把碰撞按 Skip 处理；打开身份违例仍是结果不确定的失败。

对流式回退而言，已打开 writer 的 `WriterState::Open` 不能证明目标处于隔离状态或未发生变化。
在确认 commit 或 abort 结果前发生读取、写入或 deadline 错误时，失败状态为 `Indeterminate`；commit
失败则采用其类型化 `WriteFailureState`。只有明确证明未发布时才能报告 `Unchanged`。保留恢复 writer，
以便显式清理或核查。后续结果不会改写原始复制失败的状态和统计快照，调用方须分别处理；不能仅因尚未
执行 commit 就重试。

provider 尚未交回的会话无法由核心接管。打开失败或取消之前在 provider 内部创建的资源，
仍由 provider 负责保留和回收。

## 13. 读取窗口与分配上限

`ReadOptions::validate()` 在能力检查、前缀优化和 provider I/O 之前拒绝显式
 offset + length 溢出，错误为 `InvalidOptions`；未提供 offset 时按零计算。
零长度请求仍检查或打开资源，不能吞掉不存在、权限等错误。到达或超过 EOF 的窗口为空，
跨越 EOF 时返回可读取的后缀。metadata 描述完整资源，不表示窗口大小，也不承诺快照。

同步、异步 `read_all` 和 `read_prefix` 共用可失败的几何扩容缓冲。metadata 只是提示，
即使长度极大也不会据此预分配整份对象，或在读取前拒绝实际很短的流。分配失败返回
`ResourceLimitExceeded`，并保留
底层分配错误 source。`read_all` 可以多读一个字节确认超限；`read_prefix` 不读取前缀
上限以外的探测字节。这些上限约束返回长度和消费量，不等于进程 RSS 或 provider／网络
预取上限。本地 provider 的范围能力为 Conditional，自动缩小前缀请求仍只对声明
Guaranteed `RangeRead` 的 provider 生效。

## 可选的随机读取会话

`ProviderOperation::OpenRandomAccessReader` 是可选 operation，固定值为 11，依赖已有的
`Read` capability。未实现该功能的 provider 应保持 operation 未声明：默认的
`FileSystemSpi::open_random_access_reader` 会返回 `UnsupportedOperation`，且不执行 I/O；
门面也会在分派 SPI 之前拒绝未声明的 operation。不能根据 `RangeRead` 推断支持随机读取。

实现时应只打开一次，并用同一个已打开对象上的信息构造
`OpenedRandomAccessReader` 与 `OpenedFileInfo`。此后每个偏移请求都必须继续访问该对象或
不可变版本；不能按裸路径重新 stat 来补 metadata，也不能重新打开路径来实现 `read_at`。
门面要求身份匹配、metadata 表明资源是普通文件或对象，且包含已知长度（零也有效）。目录、符号链接、
前缀、其他资源类型以及缺少长度的 metadata 都违反 provider 契约。`len()` 会缓存为本会话可读取长度上界，
但它不是内容快照，也不保证对象不会被原地修改。

`read_at` 可以短读；调用方缓冲区中只有返回计数以内的前缀有效。空缓冲区或请求范围不溢出且偏移达到
或超过打开时长度时，门面会在不调用 provider 的情况下返回 0；这两种情形都不表示观察到 EOF。
对于上界内的非空 provider 请求，返回 0 才表示该请求观察到 EOF。应保留带类型的读取错误及其 source，
不能把错误当成 EOF 或改按路径重开。异步 SPI 遵循相同
契约：Pending future 被取消后，provider 不得继续持有或修改借用的缓冲区；下一次调用仍须从
指定偏移读取同一对象。future 结束后不能有后台任务继续借用缓冲区。本指南中的最小顺序读取
provider 示例有意不声明 operation 11，仍是有效 provider；它们展示默认不支持行为，不会让
随机读取成为所有 provider 的必需方法。

## 14. 发布前检查清单

- 构造并校验一份缓存的 `ProviderProperties` 快照。
- 只添加已经实现且有理由的 operation 和 capability 强度。
- 运行共享 testkit 契约套件及 provider 专属恢复测试。
- 运行 `cargo test --locked --all-features`、doctest、clippy、rustdoc 和文档校验器。
- 审查发布、清理、取消和错误状态的行为。

## 复制与持久化结果

处理每个源条目前应先执行 `CopyOptions::max_entries()` 预算。门面对返回的已完成资源数与
`skipped` 做 checked-add 下界校验，并单独检查 `failed`，因为统计字段不能证明这几类条目互斥。
`overwritten` 是策略结果，不能再作为一个源条目累加。不能把门面的 outcome 校验当作 provider
实际扫描总量的证明。

当前本地适配器会把该限制传给 `rs-local-files`：Host 和 Rooted 复制在开始工作前先计入源根条目，
递归调度器在处理每个子条目前计费。成功结果映射已完成的文件、目录以及 skipped 和 overwritten，
但不映射 failed。这支持该适配器使用“已完成资源数加 skipped”作为下界；对于会报告 failed 的
provider，门面仍独立校验该计数。

只有原生复制尝试没有造成外部变更、也没有留下清理责任时，才能返回
`CopyAttempt::Declined`。若外部状态已变化或效果不确定，应返回带已知发布状态和统计信息的
`SpiCopyFailure`。无副作用拒绝后，门面会直接选择流式回退；原生尝试失败不能回退。
`CopyMethod::Streamed` 仅供门面拥有的流式回退使用，provider 不得在已完成的 `try_copy` outcome
中返回它。Provider 自行完成客户端流式传输时应报告 `CopyMethod::Native`；实际使用 clone、服务端
或混合策略时分别报告 `Clone`、`ServerSide` 或 `Mixed`。

`NonSensitiveMetadata` 按标准敏感性策略检查诊断键名，并在自动 `Debug` 输出中隐藏值，但会原样保存值。
Provider 在保存诊断前负责清洗原生错误、URI 和 SDK 输出；显式读取值不代表它可以安全记录。

即使 `PersistOutcome::cleanup_state()` 为 `ResidualTemporaryContainer`，该结果仍确认发布成功。
应保留成功事实，不要再次发布或清理已发布目标。portable API 不提供私有容器定位或清理重试接口，
也不承诺 provider 在后台自动清理。outcome 保留键名通过敏感性检查的 `diagnostics()`，值原样保存；
provider 在保存诊断前负责清洗值。句柄处于 `Persisted` 或 `Kept` 状态后即为终态，后续 `cleanup`、`persist`
或 `keep` 调用都会被拒绝。

## 元数据保留

`MetadataPreservePolicy` 为单个操作选择一个类别，不定义跨 provider 统一的字段集合。provider 应按操作
列出支持字段及平台、资源类型差异，并区分必需保留、尽力保留和排除字段。尽力保留字段必须处于已公开
的范围内；不能静默降低必需字段的保证。

| 策略 | 契约 |
| --- | --- |
| `None` | 不请求显式复制源 metadata；原生 copy 或 rename 仍可能使目标获得固有属性。 |
| `Portable` | 请求该操作已公开说明的、可设置的通用属性类别；不承诺跨 provider 存在统一的非空字段集合。 |
| `UserMetadata` | 请求受支持的用户自定义 metadata；不会自动包含 `Portable` 或 provider-native metadata。 |
| `ProviderNative` | 请求 provider 可表示的原生类别；应说明支持范围与尽力处理边界。 |
| `All` | 请求该操作可表示类别的并集；应明确排除不可表示字段。它不复制资源身份、生成的版本号或全部只读观测值。 |

请求类别不受支持时，如能在 I/O 前判定，应返回 `RequirementNotMet`。I/O 中设置字段失败时必须保留真实错误
和已知发布事实。`CopyOutcome::metadata()` 是 provider 对完成情况的声明；门面只比较策略枚举，不会读回并逐字段
核验目标。`PersistOptions` 是请求，`PersistOutcome` 没有字段级保留证据。两个 outcome 都不应被文档描述为
逐字段证明。默认值仍是 `CopyOptions::None` 和 `PersistOptions::Portable`。

当前 local adapter 的 **copy** 映射如下：

| 策略 | Local copy 映射 |
| --- | --- |
| `None` | Native `None` |
| `Portable` | Native `Permissions` |
| `UserMetadata`、`ProviderNative`、`All` | `RequirementNotMet` |

权限的具体平台表示由 `rs-local-files` 定义。此映射不承诺保留时间戳、owner、ACL、xattr 或 content type。
不能将此表用于 local persist：其选项分别映射 overwrite 和创建父目录，outcome 也不证明字段保留。
原生 rename 行为不是策略保证；业务依赖字段时应查阅该操作的 provider 文档并检查目标。

## 15. 延伸阅读

- [English user guide](user_guide.md) · [中文用户指南](user_guide.zh_CN.md)
- [English design](file_system_design.md) · [中文设计文档](file_system_design.zh_CN.md)
- [API 文档](https://docs.rs/qubit-fs)
