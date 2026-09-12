# Qubit FS 用户手册

[English](user_guide.md) | 简体中文

本手册针对 `qubit-fs` `0.8`、Rust 1.94 及以上版本。读者是通过已配置文件系统发布报告，
并需要在写入、复制或取消未正常完成时保留恢复信息的 Rust 应用开发者。

## 手册目标与读者

适合阅读本手册的场景包括：

1. 业务逻辑只通过 `FileSystem` 或 `AsyncFileSystem` 读写，而不嵌入 provider SDK 细节；
2. 读取、列举和复制都使用明确的选项与资源上限；
3. 部分写入、复制或取消后，需要保留类型化的发布与清理事实；以及
4. 根据这些事实选择重试、cleanup 或只读核查，而不是仅凭错误种类推断安全性。

本手册覆盖 `qubit_fs` 公共 API 及面向应用的恢复模式，不替代 provider 指南，也不对
公共 API 与测试无法证明的行为作出承诺。

## 概念模型

`FileSystem` 和 `AsyncFileSystem` 是应用使用的具体门面。存储提供者通过 `qubit_fs::spi`
实现后端；发现、配置和凭据接入由 `qubit-fs-registry` 负责。核心库不附带后端，也不依赖异步
运行时。`qubit-fs-local` 提供同步的本机文件系统，可配置为整个主机命名空间或指定根目录。

`FileSystemProperties` 保存身份、有效能力、限制、路径约束和符号链接策略的不可变快照，
读取快照不执行 I/O。registry 解析成功只代表静态约束合法，不能证明资源存在或后续 I/O 必然成功。

| 概念 | 含义 |
| --- | --- |
| `Path` | 某个已配置文件系统中的逻辑名称；层级路径和对象键有不同语义。 |
| `Uri` | 结合文件系统上下文解释的规范位置；允许仅含用户名的 authority，拒绝密码、敏感查询字段和 fragment。 |
| `ConnectionUri` | 可携带凭据的配置入口；`Display` 和 `Debug` 会脱敏。 |
| 发布 | 请求写入的目标数据是否已经可见。 |
| 清理 | 暂存或源资源是否已经释放；清理完成不代表目标已回滚。 |
| 恢复句柄 | 仍需处理的会话所有权；没有句柄不代表没有副作用。 |

## 实战场景：写入并读取本地报告

假设批处理任务向已配置的存储写入状态报告，在字节上限内读回，并通过列举确认条目可见。
成功标准包括：

- 业务逻辑只使用公共门面与逻辑路径；
- 读取遵守显式字节上限，而不是按整对象无界分配；
- 列举以增量方式观察到已发布条目；以及
- 若后续发布失败，应用可以先检查保留的事实，再决定重试或 cleanup。

下文从最小安装与配置开始，依次说明本地报告路径、复制与异步恢复、错误决策和运行限制。

## 安装与最小配置

默认 feature 集为空，只提供同步 API。异步应用显式配置
`qubit-fs = { version = "0.8", features = ["async"] }`，并使用自己已有的执行器；库不要求 Tokio。

运行本手册中的本地示例需要：

```toml
[dependencies]
qubit-fs = "0.8"
qubit-fs-local = "0.8"
tempfile = "3"
```

## 核心工作流

### 列举范围与有界读取

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

### 写入并读取一份报告

把以下代码保存到 `src/main.rs`，执行 `cargo run`。它在独立的临时根目录内写入报告，读取时
最多接收 1024 字节，最后输出 `report ready`。`tempfile` 仅用于示例目录；实际应用应使用
配置好的已有根目录，并按工作负载设置资源预算。

<!-- example: quick-start -->
```rust
use qubit_fs::Path;
use qubit_fs::directory::ListScope;
use qubit_fs::read::ReadOptions;
use qubit_fs::write::WriteOptions;
use qubit_fs_local::LocalFileSystems;
use qubit_fs_local::LocalResourcePolicy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let policy = LocalResourcePolicy::standard();
    let filesystem = LocalFileSystems::rooted(directory.path(), policy)?;
    let path = Path::parse("/report.txt")?;
    filesystem.write_all(&path, b"report ready", WriteOptions::default())?;
    let bytes = filesystem.read_all(&path, ReadOptions::default(), 1024)?;
    assert_eq!(b"report ready", bytes.as_slice());
    let scope = ListScope::Path(Path::root());
    let mut entries = filesystem.list(&scope, Default::default())?;
    assert_eq!(entries.next_entry()?.expect("published report").path, path);
    assert!(entries.next_entry()?.is_none());
    println!("{}", String::from_utf8(bytes)?);
    Ok(())
}
```

逻辑路径 `/report.txt` 位于所配置的根目录内，不是主机的 `/report.txt`。如果输入来自本机
路径 API，应使用 `qubit-fs-local` 的 `host_path_to_logical` 转换，不能直接把任意操作系统
路径文本当作逻辑路径解析。

较大数据应使用 `open_reader` 或 `open_writer` 分块传输。`read_all` 必须设置字节上限，
范围读取按选中范围而非完整 metadata 长度计算；`read_prefix` 限制返回的前缀。
writer 必须显式提交，flush 成功本身不代表已经发布。

## 进阶用法

### 复制报告并保留恢复责任

下面的应用辅助函数复制一份已完成的报告。失败时返回原始 `CopyFailure`，保留发布状态、
部分统计和可能存在的 writer；abort 失败另存为 `cleanup_error`。调用方应持续持有恢复对象，
直到完成核查或清理决策。函数不会自动重新执行复制。

<!-- example: sync-recovery -->
```rust
use qubit_fs::FileSystem;
use qubit_fs::Path;
use qubit_fs::copy::CopyFailure;
use qubit_fs::copy::CopyOptions;
use qubit_fs::copy::CopyOutcome;
use qubit_fs::error::FsError;
use qubit_fs::write::WriterRecovery;

#[derive(Debug)]
pub struct CopyRecovery {
    pub failure: CopyFailure,
    pub cleanup_error: Option<FsError>,
}

pub fn copy_report(filesystem: &FileSystem, source: &Path, target: &Path) -> Result<CopyOutcome, CopyRecovery> {
    match filesystem.copy(source, target, CopyOptions::default()) {
        Ok(outcome) => Ok(outcome),
        Err(mut failure) => {
            // Abort handles the retained session; it does not promise target rollback.
            let cleanup_error = match failure.recovery_mut() {
                Some(WriterRecovery::Opened(writer)) => writer.abort().err(),
                Some(WriterRecovery::Rejected(writer)) => writer.abort().err(),
                None => None,
            };
            // Preserve the publication facts and writer even when cleanup fails.
            Err(CopyRecovery { failure, cleanup_error })
        }
    }
}
```

复制成功后，可通过 `filesystem.list(&ListScope::Path(path.clone()), ListOptions::default())` 获取目录流，并在有界
循环内调用 `next_entry()`；`ListOptions` 从 `qubit_fs::directory` 导入。条目逐步返回，
后续读取可能失败，应先记录当前条目的处理进度。目录流不是原子快照。

层级子树过滤使用 `ListFilter::Subtree`。平面对象键使用
`ListOptions::object_keys().with_filter(ListFilter::LiteralPrefix(...))` 按原始文本匹配，
不解码或规范化键，且要求根非空。提供者无法忠实表达该请求时可以拒绝。

### 异步写入与取消

`begin_write_all` 消费 `Vec<u8>`，返回持有文件系统 clone、路径、选项和数据的 operation。
借用数据需显式 `to_vec()`；需要分块传输时，直接管理 `AsyncFileWriter` 生命周期。
旧异步 `write_all` 已删除，没有兼容包装。

下方完整辅助函数接收应用的取消 future。请通过 `cancel` 参数取消写入，并让辅助函数继续
完成恢复；不要在清理期间再丢弃整个辅助函数的 future。operation 位于执行 future 的取消
作用域外。同一次 poll 中执行和取消同时就绪时，优先接收执行结果。

<!-- example: async-recovery -->
```rust
use std::future::Future;
use std::future::poll_fn;
use std::pin::pin;
use std::task::Poll;

use qubit_fs::AsyncFileSystem;
use qubit_fs::Path;
use qubit_fs::directory::ListFilter;
use qubit_fs::directory::ListOptions;
use qubit_fs::directory::ListScope;
use qubit_fs::error::FsError;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::write::AsyncWriteAllOperation;
use qubit_fs::write::AsyncWriterRecovery;
use qubit_fs::write::AsyncWriteAllOperationFailure;
use qubit_fs::write::WriteOptions;

#[derive(Debug)]
pub struct WriteRecovery {
    pub operation: AsyncWriteAllOperation,
    pub primary: Option<AsyncWriteAllOperationFailure>,
    pub cleanup_error: Option<FsError>,
}

#[derive(Debug)]
pub enum WriteReportError {
    Preflight(AsyncWriteAllOperationFailure),
    Recovery(Box<WriteRecovery>),
}

pub async fn write_report(
    filesystem: &AsyncFileSystem,
    path: Path,
    bytes: Vec<u8>,
    cancel: impl Future<Output = ()>,
) -> Result<WriteOutcome, WriteReportError> {
    let mut operation = filesystem
        .begin_write_all(path, bytes, WriteOptions::default())
        .map_err(WriteReportError::Preflight)?;
    // Only the execute future enters the cancellation scope.
    let result = {
        let mut execution = pin!(operation.execute());
        let mut cancellation = pin!(cancel);
        poll_fn(|context| {
            if let Poll::Ready(result) = execution.as_mut().poll(context) {
                return Poll::Ready(Some(result));
            }
            if cancellation.as_mut().poll(context).is_ready() {
                return Poll::Ready(None);
            }
            Poll::Pending
        })
        .await
    };
    let primary = match result {
        Some(Ok(outcome)) => return Ok(outcome),
        Some(Err(failure)) => Some(failure),
        None => None, // The operation now records cancellation facts.
    };
    let cleanup_error = match operation.recovery() {
        Some(AsyncWriterRecovery::Opened(writer)) => writer.abort_async().await.err(),
        Some(AsyncWriterRecovery::Rejected(writer)) => writer.abort_async().await.err(),
        None => None,
    };
    // Published means do not resend. Indeterminate requires reconciliation,
    // including when no writer was returned. A missing stat result is not
    // proof that a remote request cannot finish later.
    Err(WriteReportError::Recovery(Box::new(WriteRecovery {
        operation,
        primary,
        cleanup_error,
    })))
}

/// Lists report keys across a configured flat namespace with a bounded result set.
pub async fn list_reports(filesystem: &AsyncFileSystem) -> Result<Vec<Path>, FsError> {
    let scope = ListScope::Namespace;
    let options = ListOptions::object_keys()
        .with_filter(Some(ListFilter::LiteralPrefix("reports/".to_owned())))
        .with_max_entries(Some(1000));
    let mut stream = filesystem.list(&scope, options).await?;
    let mut paths = Vec::new();
    while let Some(entry) = stream.next_entry_async().await? {
        paths.push(entry.path);
    }
    Ok(paths)
}
```

显式错误保存在 `primary` 中；取消时 `primary == None`，状态保存在 `operation`。
abort 失败后，返回对象同时保留 `cleanup_error` 和 writer。没有 writer 时，仍可通过
`operation.filesystem()` 和 `operation.path()` 核查目标。没有句柄，或一次 `stat` 返回
`NotFound`，都不能证明远端请求稍后绝不会完成；核查方式取决于提供者的请求标识与完成保证。

| 执行事件 | 保留的信息 |
| --- | --- |
| 丢弃未轮询的执行 future | 保持 `Ready`，未调用提供者，数据仍由 operation 持有。 |
| 执行开始后取消 | `Failed(Indeterminate)`，保留已确认字节数和已取得的 writer。 |
| 成功 | `Completed`，字节数准确，释放已提交 writer 和原始数据。 |
| 显式失败 | 保留类型化发布状态、确认字节数和必要的恢复 writer。 |
| 再次执行 | `InvalidState`，不再调用提供者，历史状态和计数不变。 |
| 取走恢复 writer | 转移处理责任；后续恢复不改写 operation 的历史快照。 |

`begin_copy` 对异步流式复制采用相同的所有权原则。取消时持续持有 operation。
`Drop` 不创建执行器，也不承担必须确认完成的异步清理；需要确认时显式 await。

## 错误与诊断

### 保证与错误决策

打开 writer 失败时，只有提供者明确报告 `FsEffectState::Unchanged` 且没有不确定错误，
才能证明没有副作用。缺少 effect、`Applied`、`PartiallyApplied` 或不确定证据，都让整次
write/copy 归为 `Indeterminate`。打开步骤已生效不代表整文件已发布。
`AlreadyExists + Skip` 也必须有明确的无副作用证据。调用提供者前的本地校验失败可确定未发布。

提交失败继续保留提交阶段的发布事实。`Published` 不能作为重发依据，但仍可能需要清理。
不要只凭错误名称判断能否重试。`exists` 仅对 `NotFound` 返回 false；权限、认证、超时和 I/O
错误都会继续返回错误。

请求区分尽力而为和必须满足的保证。`WriteOutcome`、`RenameOutcome` 和 `CopyOutcome`
报告 durability 信息；`PersistOutcome` 报告发布和清理，但没有 durability 字段，临时资源
持久化成功本身不能证明已完成持久同步。原子性与持久性不同，二者不能互相
推导。静态不可能满足的 required 保证会在 I/O 前拒绝，完成后还会核验实际 outcome；已知发布
之后发现保证不满足，不能把结果改写为未产生变化。

没有原生 copy，或提供者明确无副作用地拒绝 fast path 后，门面可以执行允许范围内的普通文件
流式复制；原生复制失败不会触发自动重试。要求服务端、原子或持久复制、目录树模式，以及改变
符号链接策略的请求不能使用该 fallback。`CopyOptions::deadline` 是从 operation 构造时开始
累计的协作式预算，在阶段边界检查，不是能打断任意 Pending future 的定时器。提供者错误仍是主错误。

临时文件和目录有独立的所有权生命周期，提供 `cleanup`、`keep` 和 `persist`。0.8 的
恢复快照包含两个相互独立的维度：保留的目标发布事实，以及当前源资格。

| `PersistFailureState` | 保留的目标事实 | 源资格 | 安全的下一步 |
| --- | --- | --- | --- |
| `NotPublished` | 未发布 | 仍拥有 | 修正目标后重试，或清理 |
| `NotPublishedSourceIndeterminate` | 未发布 | 不确定 | 只读核查；不得删除可能的替换物 |
| `NotPublishedSourceCleanupRequired` | 未发布 | 需要清理 | 只重试 cleanup |
| `NotPublishedSourceReleased` | 未发布 | 已释放 | 不再执行源操作 |
| `PublishedSourceRetained` | 已发布 | 需要清理 | 保留目标，只清理残留源 |
| `PublishedSourceIndeterminate` | 已发布 | 不确定 | 保留目标事实，只读核查源 |
| `PublishedSourceReleased` | 已发布 | 已释放 | 不得再次发布 |
| `Indeterminate` | 不确定 | 不确定 | 核查后处理，不自动重试 |

`PersistFailure::publication_target()` 表示最近一次明确确认的目标，而不只是失败调用传入的
目标。资源先前已经发布后，非法重试会在 provider I/O 前被拒绝；门面错误仍可能保留先前的
`PublishedSource*` 状态和目标，但这份快照不表示重试又发布了一次。cleanup 错误和异步
取消同样保留目标历史。源资格一旦不确定，后续路径或选项错误不能把它恢复为可拥有状态。
选择重试、cleanup 或只读核查前，应同时查看 `failure.state()`、
`failure.publication_target()` 和句柄的 `TempResourceState`。

### 打开失败与恢复协议

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

## 排障

- 找不到文件系统：配置后端或通过 registry 取得门面，核心库不会自行选择提供者。
- 保证不受支持：核对有效能力和请求选项，再由应用决定更换提供者或调整要求。
- 列举中途失败：保存已处理进度，并考虑后端一致性，不要假定快照语义。
- 写入或复制结果不确定：持续持有 operation 和错误，先判断发布事实，再核查，避免隐式重试。
- URI 被拒绝：凭据留在配置边界；`expose_unredacted` 仅用于受控消费，不能用于日志或缓存键。
- 存在私有敏感字段：传入显式脱敏策略，标准策略仍是不可移除的底线；提供者应在构造规范 `Uri` 前移除未识别的私有凭据。
- 内存和资源：`Vec` 所有权使整文件内存成本可见。为读取、列举、复制和临时资源设置适当限制；声明的限制不是并发请求的总配额。

## 限制与最佳实践

可移植契约不会把对象键变成层级路径，不保证所有后端支持全部能力，也不实现跨文件系统 move。
平台行为和根目录权限边界由具体后端提供。

### 读取窗口与分配上限

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

## 延伸阅读

- [README](../README.zh_CN.md)
- [English user guide](user_guide.md)
- [架构设计](file_system_design.zh_CN.md)
- [English architecture](file_system_design.md)
- [docs.rs API 文档](https://docs.rs/qubit-fs)
- [仓库地址](https://github.com/qubit-ltd/rs-fs)
