# Qubit FS 用户指南

[English guide](user_guide.md) · [README](../README.zh_CN.md)

本指南适用于 `qubit-fs` 0.4、Rust 1.94 及以上版本，面向通过已配置文件系统发布报告的应用。
重点说明正常读写流程，以及写入、复制或取消未正常结束时，如何保留恢复所需的信息和资源。

## 概念与配置

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

默认 feature 集为空，只提供同步 API。异步应用显式配置
`qubit-fs = { version = "0.4", features = ["async"] }`，并使用自己已有的执行器；库不要求 Tokio。

运行下方本地示例需要：

```toml
[dependencies]
qubit-fs = "0.4"
qubit-fs-local = "0.4"
tempfile = "3"
```

## 写入并读取一份报告

把以下代码保存到 `src/main.rs`，执行 `cargo run`。它在独立的临时根目录内写入报告，读取时
最多接收 1024 字节，最后输出 `report ready`。`tempfile` 仅用于示例目录；实际应用应使用
配置好的已有根目录，并按工作负载设置资源预算。

<!-- example: quick-start -->
```rust
use std::time::Duration;

use qubit_fs::Path;
use qubit_fs::read::ReadOptions;
use qubit_fs::write::WriteOptions;
use qubit_fs_local::LocalCopyResourceLimits;
use qubit_fs_local::LocalDeleteResourceLimits;
use qubit_fs_local::LocalFileSystems;
use qubit_fs_local::LocalListResourceLimits;
use qubit_fs_local::LocalResourcePolicy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let list_budget = LocalListResourceLimits::new(8, 64, 4 * 1024, 4, Duration::from_secs(5))?;
    let copy_budget = LocalCopyResourceLimits::new(8, 64, 1024 * 1024, 4, Duration::from_secs(5))?;
    let delete_budget = LocalDeleteResourceLimits::new(8, 64, 4 * 1024, Duration::from_secs(5));
    let policy = LocalResourcePolicy::bounded(list_budget, copy_budget, delete_budget);
    let filesystem = LocalFileSystems::rooted(directory.path(), policy)?;
    let path = Path::parse("/report.txt")?;
    filesystem.write_all(&path, b"report ready", WriteOptions::default())?;
    let bytes = filesystem.read_all(&path, ReadOptions::default(), 1024)?;
    assert_eq!(b"report ready", bytes.as_slice());
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

## 复制报告并保留恢复责任

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
            let cleanup_error = match failure.writer_mut() {
                Some(writer) => writer.abort().err(),
                None => None,
            };
            // Preserve the publication facts and writer even when cleanup fails.
            Err(CopyRecovery { failure, cleanup_error })
        }
    }
}
```

复制成功后，可通过 `filesystem.list(path, ListOptions::default())` 获取目录流，并在有界
循环内调用 `next_entry()`；`ListOptions` 从 `qubit_fs::directory` 导入。条目逐步返回，
后续读取可能失败，应先记录当前条目的处理进度。目录流不是原子快照。

层级子树过滤使用 `ListFilter::Subtree`。平面对象键使用
`ListOptions::object_keys().with_filter(ListFilter::LiteralPrefix(...))` 按原始文本匹配，
不解码或规范化键，且要求根非空。提供者无法忠实表达该请求时可以拒绝。

## 异步写入与取消

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
use qubit_fs::error::FsError;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::write::AsyncWriteAllOperation;
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
    let cleanup_error = match operation.recovery_writer() {
        Some(writer) => writer.abort_async().await.err(),
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

## 保证与错误决策

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

临时文件和目录有独立的所有权生命周期，提供 `cleanup`、`keep` 和 `persist`。
持久化失败包含五种状态：`NotPublished`、`NotPublishedSourceReleased`、
`PublishedSourceRetained`、`PublishedSourceReleased`、`Indeterminate`。
`PersistFailure::publication_target()` 保留已确认的发布目标；源资源已释放不等于没有发布。

## 排障与运行限制

- 找不到文件系统：配置后端或通过 registry 取得门面，核心库不会自行选择提供者。
- 保证不受支持：核对有效能力和请求选项，再由应用决定更换提供者或调整要求。
- 列举中途失败：保存已处理进度，并考虑后端一致性，不要假定快照语义。
- 写入或复制结果不确定：持续持有 operation 和错误，先判断发布事实，再核查，避免隐式重试。
- URI 被拒绝：凭据留在配置边界；`expose_unredacted` 仅用于受控消费，不能用于日志或缓存键。
- 存在私有敏感字段：传入显式脱敏策略，标准策略仍是不可移除的底线；提供者应在构造规范 `Uri` 前移除未识别的私有凭据。
- 内存和资源：`Vec` 所有权使整文件内存成本可见。为读取、列举、复制和临时资源设置适当限制；声明的限制不是并发请求的总配额。

可移植契约不会把对象键变成层级路径，不保证所有后端支持全部能力，也不实现跨文件系统 move。
平台行为和根目录权限边界由具体后端提供。

## 延伸阅读

- [0.4 迁移指南](migration_0_4.zh_CN.md)
- [架构设计](file_system_design.zh_CN.md)
- [API 文档](https://docs.rs/qubit-fs)
