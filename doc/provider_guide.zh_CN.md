# Provider 实现指南

[English version](provider_guide.md)

本指南面向 `qubit-fs` 0.4 的 provider 作者，说明如何实现一个能够被
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

## 8. 异步取消

异步操作必须在取消域之外持有操作状态，并一直保留 operation，直到完成清理和恢复
决策。不要让取消丢弃 writer、临时路径或发布结果。

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
- [0.4 迁移指南](migration_0_4.zh_CN.md)
- [API 文档](https://docs.rs/qubit-fs)
