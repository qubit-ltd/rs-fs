# Provider 实现指南

[English version](provider_guide.md)

本指南面向 `qubit-fs` 0.7 的 provider 作者，说明如何实现一个能够被
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
要求基础能力被支持；`Guaranteed` 派生能力要求基础能力也是 `Guaranteed`。门面会在
provider 使用前拒绝不一致的声明。

## 5. 一个完整的最小 provider

下面的 provider 暴露一个只读健康资源。这段完整代码由文档 fixture 编译验证。

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

pub struct HealthProvider {
    properties: ProviderProperties,
}

impl HealthProvider {
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

    fn validate_path(path: &Path, operation: FsOperation) -> FsResult<()> {
        if path.as_str() == "/health" {
            Ok(())
        } else {
            Err(FsError::new(
                FsErrorKind::NotFound,
                operation,
                "health resource not found",
            )
            .with_path(path.clone()))
        }
    }
}

impl FileSystemSpi for HealthProvider {
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }

    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Self::validate_path(request.path(), FsOperation::Stat)?;
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(2)),
        ))
    }

    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Self::validate_path(request.path(), FsOperation::OpenReader)?;
        Ok(OpenedReader::new(
            OpenedFileInfo::new(self.properties.info().id().clone(), request.path().clone()),
            Box::new(Cursor::new(b"ok".to_vec())),
        ))
    }
}

pub fn read_health() -> FsResult<Vec<u8>> {
    let filesystem = FileSystem::from_spi(HealthProvider::new()?)?;
    filesystem.read_all(&Path::parse("/health")?, ReadOptions::default(), 16)
}
```

## 6. 错误与上下文

返回 `FsError` 时带上失败的操作和路径。门面会保留操作的 `FsEffectState`（例如发布
是否可能已经对外可见），让调用方能够基于事实选择恢复策略，而不是猜测。只要知道
目标路径或临时路径，就应将其放进错误上下文。

## 7. 发布与清理

writer 和临时资源实现必须分别报告发布、清理和恢复失败。清理失败并不表示目标一定
不存在。遵循 `CopyOutcome` 与写入恢复契约，保留主失败以及清理错误，便于观测和处置。

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

## 9. 接入 `qubit-fs-testkit`

适配器应运行 `qubit-fs-testkit` 的共享契约套件，并针对限制、capability、路径规则
和恢复补充后端测试。发布前运行 fixture 和 all-features 测试。

## 10. 常见契约违规

测试失败时，先检查 operation 与 capability 是否只声明了已实现的内容、capability
依赖强度是否正确、stat 元数据是否与操作结果一致，以及每个错误是否包含路径和操作
上下文。只有 `stat` 明确返回 `NotFound` 时，`exists` 才会返回 false；权限、认证、
超时和 I/O 错误必须继续作为错误返回。

## 11. 发布前检查清单

- 构造并校验一份缓存的 `ProviderProperties` 快照。
- 只添加已经实现且有理由的 operation 和 capability 强度。
- 运行共享 testkit 契约套件及 provider 专属恢复测试。
- 运行 `cargo test --locked --all-features`、doctest、clippy、rustdoc 和文档校验器。
- 审查发布、清理、取消和错误状态的行为。

## 12. 延伸阅读

- [English user guide](user_guide.md) · [中文用户指南](user_guide.zh_CN.md)
- [English design](file_system_design.md) · [中文设计文档](file_system_design.zh_CN.md)
- [API 文档](https://docs.rs/qubit-fs)

## 列举范围与有界读取

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

## 0.6 的打开失败与恢复协议

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
不确定的 abort 结果不算完成确认。隔离句柄的 Drop 不启动清理，也不调用
`cancel_on_drop`。清理必须依据 session 实际拥有的资源，不能依据未校验的诊断路径。
公开清理错误只使用已配置的 provider 和已知请求路径；没有 parent 的临时请求不伪造路径。
主失败和清理错误应同时保留。

整文件写入和复制通过 `WriterRecovery` / `AsyncWriterRecovery` 交回资源：
`Opened` 是已验证 writer，`Rejected` 只有清理权限。用 `recovery`、适用时的
`recovery_mut` 及 `take_recovery` 替代旧的 writer 专用访问器。
同步 `WriteAllFailure::state()` 和 `written_bytes()` 保存失败时的事实，包括短写确认
字节数和确切提交状态；abort 或取走会话都不改写历史。`into_parts` 返回错误、状态、
已确认字节数和恢复会话。对于打开阶段的 copy 碰撞，只有 provider 打开失败、明确证明无副作用且没有隔离会话
时，才能把碰撞按 Skip 处理；打开身份违例仍是结果不确定的失败。

provider 尚未交回的会话无法由核心接管。打开失败或取消之前在 provider 内部创建的资源，
仍由 provider 负责保留和回收。

## 读取窗口与分配上限

`ReadOptions::validate()` 在能力检查、前缀优化和 provider I/O 之前拒绝显式
 offset + length 溢出，错误为 `InvalidOptions`；未提供 offset 时按零计算。
零长度请求仍检查或打开资源，不能吞掉不存在、权限等错误。到达或超过 EOF 的窗口为空，
跨越 EOF 时返回可读取的后缀。metadata 描述完整资源，不表示窗口大小，也不承诺快照。

同步、异步 `read_all` 和 `read_prefix` 共用可失败的几何扩容缓冲。metadata 只是提示，
即使长度极大也不会据此预分配整份对象。分配失败返回 `ResourceLimitExceeded`，并保留
底层分配错误 source。`read_all` 可以多读一个字节确认超限；`read_prefix` 不读取前缀
上限以外的探测字节。这些上限约束返回长度和消费量，不等于进程 RSS 或 provider／网络
预取上限。本地 provider 的范围能力为 Conditional，自动缩小前缀请求仍只对声明
Guaranteed `RangeRead` 的 provider 生效。
