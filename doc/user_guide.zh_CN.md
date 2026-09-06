# Qubit FS 用户指南

`qubit-fs` 0.3.0 是适用于 Rust 1.94 及以上版本的 provider-neutral 文件系统抽象。它提供
同步和异步应用门面，但不内置存储后端，也不绑定异步 runtime。

## 目的与读者

本指南面向需要访问已配置 filesystem、但不希望把文件操作耦合到 provider 实现的应用开发者。
它说明如何使用 `FileSystem` 与 `AsyncFileSystem`，以及如何根据可恢复失败的状态作出安全
决策。

provider 作者在 `qubit_fs::spi` 中实现扩展契约。provider 的发现、配置和凭据由
`qubit-fs-registry` 负责；核心 crate 不会自行发现 provider，也不会自行选择 runtime。

## 概念模型

```text
应用
  │  使用 FileSystem / AsyncFileSystem 和逻辑 Path
  ▼
qubit-fs 门面 ───────────────► handle、option、outcome、typed failure
  │
  └── qubit_fs::spi ◄──────── provider 实现

qubit-fs-registry ───────────► provider 发现、配置、凭据
```

一个 `FileSystem` 或 `AsyncFileSystem` 代表一次完成配置的 filesystem。endpoint、bucket、
root、region 或 credential profile 不同，同一 provider 也可以产生多个门面。
`FileSystemProperties` 是 identity、capability、limit、路径约束与 provider 符号链接策略的不可变
快照，读取它不执行 I/O。`ListOptions` 和 `CopyOptions` 可以按操作覆盖
`SymlinkPolicy`；可移植抽象提供 `Reject` 与 `FollowWithinFileSystem`，provider 负责把后者映射到
自己的 namespace 或 rooted 边界。

每个门面操作都会在 provider I/O 前，按 configured filesystem 的 path semantics、允许的
form 和声明的 limits 校验 `Path`。registry resolution 使用相同的静态校验；resolution
构造成功不代表已经执行 `stat` 或其他 I/O。

### 名称、地址与 secret

| 类型 | 职责 | 凭据边界 |
| --- | --- | --- |
| `Path` | 一个已配置 filesystem 内的已验证逻辑名称 | 不是跨 filesystem 地址；门面操作还会按该 filesystem 的路径约束校验。 |
| `Uri` | 用于持久化和选择的、不含 secret 的 canonical 资源位置 | 只能在 configured filesystem 上下文中解释；拒绝 userinfo、credential-like query field 与 fragment，并保留 RFC 3986 词法差异。 |
| `ConnectionUri` | registry/配置入口 | 可以携带连接凭据，但 `Display` 与 `Debug` 会脱敏；不得记录或持久化原始连接文本。 |

普通 `Uri` 只有结合 configured filesystem 上下文才表示资源位置。它不包含 filesystem
identity，不能独立解析为跨 provider 位置；registry resolution 会把门面、provider-local
`Path` 和 canonical `Uri` 绑定在一起。

`ConnectionUri` 让 registry/provider 在受控边界消费凭据，再生成安全的 canonical `Uri`。
它可以在受控处理期间内部保留原始文本，但只有移除全部敏感 component 后，`try_to_uri`
才会成功。`expose_unredacted` 只用于同一个受控边界，不能把结果送入日志、序列化、metadata、
错误消息或 cache key。

`qubit-fs` 无法识别的 provider 私有凭据字段仍由 provider 负责：provider 必须在构造
canonical `Uri` 前消费或移除这些字段，并在自己的凭据处理边界内保存 secret value。

默认解析使用固定的标准脱敏策略。这是不可移除的安全底线：`parse_with_policy` 始终应用该
底线，自定义策略只能增加 provider-specific 的敏感 query 名称，不能让标准敏感 component
变得可接受。应用如果还有额外的敏感 query 名称，必须通过 `Uri::parse_with_policy` 或
`ConnectionUri::parse_with_policy` 显式传入策略快照；`ConnectionUri` 还会用保存的策略完成
后续 secret 分类和脱敏格式化。

## 安装与取得门面

```toml
[dependencies]
qubit-fs = "0.3.0"
```

`async` feature 默认启用。只使用同步 API 的应用可以关闭默认 feature，避免编译异步门面和
SPI：

```toml
qubit-fs = { version = "0.2.0", default-features = false }
```

通过 provider 配置或 registry 集成取得已配置的 `FileSystem` 或 `AsyncFileSystem`。下列公共
构造边界适用于实现 provider 或编写聚焦测试：

```rust,ignore
use qubit_fs::{FileSystem, Path, ReadOptions};
use qubit_fs::spi::FileSystemSpi;

fn inspect<S: FileSystemSpi + 'static>(provider: S) -> qubit_fs::FsResult<()> {
    let fs = FileSystem::from_spi(provider)?;
    let report = Path::parse("/reports/2026/summary.csv")?;
    let _metadata = fs.stat(&report)?;
    let _reader = fs.open_reader(&report, ReadOptions::default())?;
    Ok(())
}
```

应用代码应停留在 `FileSystem` 和 `AsyncFileSystem`；provider trait、request、session 与
envelope 都位于 `qubit_fs::spi`。

## 真实场景：发布日报

假设一个任务需要把完成的报告复制到发布位置，再枚举发布目录供下游处理。失败后的决策取决于
typed state 已证明了什么，而不仅是是否返回了 error。

### 同步工作流

```rust,ignore
use qubit_fs::{CopyOptions, FileSystem, ListOptions, Path};

fn publish(fs: &FileSystem, source: &Path, release_dir: &Path) -> qubit_fs::FsResult<()> {
    let target = Path::parse("/releases/2026-07-30/summary.csv")?;

    match fs.copy(source, &target, CopyOptions::default()) {
        Ok(_outcome) => {}
        Err(failure) => {
            // 记录 failure.state() 与 failure.partial_stats()。若保留 writer，
            // 在其 state 得到处理前继续持有它。
            let (error, _, _, _) = failure.into_parts();
            return Err(error);
        }
    }

    let mut entries = fs.list(release_dir, ListOptions::default())?;
    while let Some(entry) = entries.next_entry()? {
        // 每次处理一个条目，并由应用设置数量上限。
        let _path = entry.path;
    }
    Ok(())
}
```

`DirectoryStream::next_entry` 按条目增量枚举。它避免把目录无界地收集到内存，也意味着已处理
部分条目后仍可能发生错误。下游工作应具备幂等性，或在请求下一条目前记录检查点。

`open_writer` 返回 `FileWriter`；写入字节后调用 `commit`。commit 失败会返回
`WriteFailure`，其 state 区分 retryable/not-published、published 与 indeterminate。
`write_all` 在需要恢复时会通过 typed `WriteAllFailure` 保留 writer。`rename` 返回
`RenameFailure`；`copy` 返回 `CopyFailure`，其中包含 partial statistics，并会在适用时保留
recovery writer。重试、`abort`、cleanup 或核对 source/target 前，都应先检查 state。
`abort` 成功会返回 `WriteAbortOutcome`；cleanup 完成并不等于 destination 未发布，
调用者仍需检查其中的 `NotPublished`、`Published` 或 `Indeterminate`。

writer 离开 `Open` 后再次调用 `commit` 属于非法状态，且不会再次调用 provider。返回的
failure 仍报告 writer 已知的发布事实：已 commit 或已发布的 writer 报告 `Published`，
已 abort 或未发布的 writer 报告 `NotPublished`，不确定 writer 报告 `Indeterminate`。

只要门面能在本地确定所要求的 atomicity 或其他 guarantee 无法满足，就会在产生副作用前进行
检查。Copy guarantee 会明确区分 source mode：`AtomicFileCopy` 和
`DurableFileCopy` 用于普通文件，`AtomicTreeCopy` 和 `DurableTreeCopy` 用于目录树。
write、rename 和临时资源 persist 的 outcome 不宣称 durable publication。成功
outcome 只报告其实际建模的保证。

`FileSystemCapability::Copy` 表示 provider-native copy fast path，并不是门面执行普通文件复制
的唯一方式。未声明该 capability 时，门面会直接评估 allowlist 内的流式 fallback，并要求
`Read` 与 `Write`。该 fallback 受文档规定的 copy options 限制，也不能满足 required
server-side、atomic 或 durable copy guarantee。

fallback 会在 `stat`、reader 或 writer I/O 前拒绝 `CopyMode::Tree`，也拒绝与 filesystem
默认策略不同的符号链接策略 override。缺省 override 或与默认策略相同才可进入 allowlist。
原生 `try_copy` 可以支持 Tree 或策略 override；这些限制只适用于原生能力缺失或明确
`Declined` 后的 fallback。

### 异步工作流

`AsyncFileSystem` 通过 runtime-neutral future 提供对应的门面操作。请在应用已有的 runtime
上运行它们；`qubit-fs` 不要求 Tokio、`futures-io` 或其他 executor。

异步门面也提供 `write_all`；若必须保留 `AsyncFileWriter` 以便恢复，它会返回
`AsyncWriteAllFailure`。

```rust,ignore
use qubit_fs::{AsyncFileSystem, CopyOptions, Path};

async fn publish_async(
    fs: &AsyncFileSystem,
    source: Path,
    target: Path,
) -> qubit_fs::FsResult<()> {
    let mut operation = fs.begin_copy(source, target, CopyOptions::default())?;
    match operation.execute().await {
        Ok(_outcome) => Ok(()),
        Err(failure) => {
            // 保留 operation，并检查 failure.state() 以决定恢复动作。
            let (error, _, _) = failure.into_parts();
            Err(error)
        }
    }
}
```

`begin_copy` 返回 `AsyncCopyOperation`，因为流式 copy 可能保留需要恢复的 async writer。
调用 `execute(&mut self).await`，并在恢复责任结束前保留 operation。若 execution future
已经被 poll、却在完成前被 drop，operation 会记录 indeterminate state；未被 poll 的 future
被 drop 后 operation 仍是 ready。需要确认完成时，应显式 await async writer 和临时资源的
cleanup。

`CopyOptions::deadline` 是协作式累计时间预算。同步 copy operation 或异步 `begin_copy`
handle 构造时开始计时，因此调用 `execute` 前的等待也计入预算。门面会在 native copy
前后、fallback 每轮 read/write 前后、flush 前后以及 commit 前后检查。provider 已返回的
错误会被保留，不会被 timeout 覆盖。发布前超时会保留 recovery writer；若 commit 已发布
target 后才超时，failure 必须是 `Published`，保留成功统计且不返回可重试的已完成 writer。

## 错误诊断与恢复

`FsError` 包含 error kind、operation，以及可用的逻辑 path、source、target 与 provider
context。诊断时先查看 kind 和 operation，再依据 typed failure state 选择安全的下一步。

| 场景 | API 表达的事实 | 常见下一步 |
| --- | --- | --- |
| `exists` 返回 `Ok(false)` | `stat` 明确返回 `NotFound` | 将资源视为不存在。 |
| `exists` 返回 `Err` | 原因不是 `NotFound` | 处理该错误；权限、认证、超时和 I/O 不表示不存在。 |
| copy/rename/write 失败 | typed state；copy 还提供 partial statistics，且可能保留 writer | 仅在 state 支持时重试；否则 abort、cleanup 或核对。 |
| 临时资源 persist 失败 | `PersistFailureState` 为 `NotPublished`、`PublishedSourceRetained` 或 `Indeterminate` | 保留所有权、清理 retained source，或在再次发布前对账。 |
| 所要求的保证不可用 | capability/requirement failure 可在副作用前被发现 | 更换 provider/options，或放宽要求。 |

临时文件和目录是由门面拥有的 handle。`TempFile`、`TempDirectory` 及其 async 对应类型都
提供显式 `cleanup`、`keep` 与 `persist`。可恢复失败后它们的 state 仍然有意义；不要依赖
`Drop` 完成必须确认的 I/O 操作。

## 排障

**没有可用的 filesystem。** 仅使用 `qubit-fs` 时这是预期现象：它不含后端，也不选择
provider。请通过 registry 或 provider 集成完成配置，再取得具体门面。

**URI 校验失败，或日志只显示被遮蔽的值。** 仅在配置入口使用 `ConnectionUri`，通过受控的
registry/provider 路径转换，并保存产生的 `Uri`。canonical `Uri` 不允许 userinfo、
credential-like query field 或 fragment。自定义策略不能削弱标准凭据安全底线；provider 私有
凭据必须在 provider 构造 canonical `Uri` 前移除。

**`exists` 没有返回 `false`。** 只有 `NotFound` 才映射为不存在。权限、认证、网络、超时和
I/O error 表明尚未确定资源是否存在。

**目录枚举在中途停止。** 目录读取是增量的，不是原子快照或预加载 vector。请在应用层保存进度，
然后从安全检查点恢复或重启。

**copy、write、rename 或临时发布可能已产生副作用。** 先读取 typed state。`Published` 与
`Indeterminate` 需要对账；在完成恢复决策前，应保留所有 writer 或 temp handle。

**range read 意外超过预算。** `read_all` 将 `max_bytes` 应用于选定的 range，而不是完整
资源长度。已知资源长度时，选定长度为
`min(max(resource_length - offset, 0), requested_length)`；`FileMetadata` 中仍保存完整
长度。即使 metadata 缺失或不准确，流式读取仍会执行实际字节预算检查。

## 限制与非目标

- 核心 crate 不提供本地、远程或对象存储后端。
- 它不发现 provider、不管理凭据，也不绑定 async runtime。
- 它不承诺每个 provider 支持全部操作或 guarantee。
- 它不会把 object key 强制解释为层级路径，也不会在公共契约外规范化 provider 语义。
- 它不会把增量目录枚举变成完整且原子的目录快照。

## 交叉链接

- [English README](../README.md) · [中文 README](../README.zh_CN.md)
- [English user guide](user_guide.md)
- [文件系统架构设计](file_system_design.zh_CN.md)
- [API 文档](https://docs.rs/qubit-fs)
