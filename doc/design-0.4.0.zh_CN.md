# rs-fs 0.4.0 详细设计：统一操作生命周期与通用文件能力

- 状态：待评审设计，尚未实现。
- 日期：2026-11-11。
- 当前版本：qubit-fs 0.3.0；下一破坏性版本：0.4.0。
- 依赖方向：rs-fs → rs-operation / rs-io / rs-budget；本库不依赖具体本地或网络 provider。
- 本设计补充现有 file_system_design.zh_CN.md，目标是对齐 local-files 的通用语义，保留 provider 中立性。

## 0. 实施前置条件：先实现 rs-operation

**必须先实现并验收 rs-operation 0.1.0 的基础协议，再开始本设计的库实现。** 只有设计文档或空 crate 不满足此前置条件；本轮仅修订设计，不修改实现或 Cargo 清单。

上游入口为 [rs-operation 设计](../../rs-operation/doc/design-0.1.0.zh_CN.md)。开始实施前，必须固定其已验收版本/不可变提交并验证：同步/异步观察同一操作、取消树与单调期限、子操作 reserve/attach/commit、ResourceUse、OwnedCleanup、恢复所有权、结果与释放分离、Drop 托管及 shutdown 超期继续观察。若上游缺少必需能力，应先在 rs-operation 补齐契约和测试，不在下游复制一套替代机制。

### 0.1 统一职责与接口规则

- 本库直接依赖 `qubit-operation = "^0.1"`；同步托管接口也使用它，不把通用协议仅放在 async feature 后。具体领域异步 API 仍可由本库 async feature 控制；导入 crate 或构造纯配置不启动服务线程。
- 应用创建并注入共享 `OperationSupervisor`；本库创建受管理子 scope，不因领域服务各异而再创建私有通用 supervisor。应用关闭共享服务，本库只关闭自己受理范围内的子操作；显式独占的便利构造必须说明其服务归属。
- 同步/异步共用 `OperationContext`、`OperationHandle<T,E>`、`OperationObserver`、`ResourceUse` 和 `SettlementReport`。领域包装仅增加强类型结果/领域方法，不创建另一套 token、deadline、detach、恢复注册表或调度循环。
- `result()/result_blocking()` 一次性取业务结果；`released()/released_blocking()` 观察稳定终结。等待 future Drop 不取消仍持有的 handle，handle Drop 请求停止，detach 返回公共 observer；Unconfirmed 只能观察或超期返回快照，不能视为 Released。
- 请求提交必须先预留公共登记槽和领域容量，attach 领域 control 后 commit 才能产生外部副作用。停止信号和启动竞争使用公共线性化协议，未启动受理失败归还 owned 输入。
- 领域 worker、进程 reaper、原生 Session 和领域字节预算仍由本库管理；通用 supervisor 不执行阻塞 I/O 或推理。子任务从父 scope 切分配额，不重复争抢 root 槽，也不将同一资源登记两次。
- `OperationContext` 只携带共有上下文；领域 options 保留权限、版本、原子性、输出或采样策略。总期限向下收紧，不重置；清理不复用已取消的业务 token。生命周期停止请求不撤销已发生副作用。
- 不增加进度接口、progress feature 或事件总线依赖。业务自行使用 rs-progress / rs-event-bus；本库状态与诊断仅用于正确终结，不自动上报业务阶段。

### 0.2 共用验收与发布门禁

先通过 rs-operation 核心测试，再用本库假执行器验证：取消先于 commit 不启动、业务结果早于资源释放、队列满仍能停止/清理、Unconfirmed 持有资源、恢复 owner 不丢、借用对象不逃逸、外部 driver 丢失不伪造释放、公共 handle 的同步/异步对等。以下领域专属验收在此基础上叠加，不能用领域测试替代上游前置验收。

## 1. 目的、场景与边界

rs-fs 是同步/异步文件系统 facade 和 provider SPI，不是本地文件系统实现。0.4 将当前已经支持的读写、复制、随机访问和临时资源接入 rs-operation，并将 rs-local-files 中跨 provider 有意义的能力提升为清晰的公共契约。

典型场景：从远程对象采样识别类型；把远程内容暂存后调用转换命令；将生成文件原子发布；有预算地复制目录树；对已读取版本进行条件更新。应用不应因底层由本地文件换成对象存储就丢失取消、部分副作用和恢复所有权。

“对齐”指术语、保证、失败和资源模型一致，不要求所有 provider 都能实现本地原语。Host/Rooted、非 UTF-8 原生路径、POSIX 权限和 ACL 不直接纳入通用 facade；不能为了 API 齐全对不支持的远程能力伪造成功。

## 2. 当前实现与差距

0.3 已有 FileSystem/AsyncFileSystem、FileSystemSpi/AsyncFileSystemSpi、writer/temp/目录 session、AsyncCopyOperation/AsyncWriteAllOperation，以及 FsEffectState。不能把这次工作描述成从零增加异步。

- ProviderProperties 已分离 ProviderOperation 和 Unsupported/Conditional/Guaranteed 能力；沿用并扩展，不以 boolean 代替支持强度。
- 已有 OpenRandomAccessReader 和同一打开对象的读取契约，Magika 可以受益；不能退化为每次 read_at 按路径重开。
- 现有 Async SPI 返回借用请求的 Future，Drop 停止本地 poll，不证明远程回滚；新的托管入口必须明确所有权和释放协议。
- copy/write-all 有自己的状态、恢复快照和 execute 协议；迁移应保留领域状态，通用取消/观察/后台终结交给 operation。
- local-files 有更完整的一致性观测、暂存/密封、条件发布和目录树发布。需要区分通用可提升能力和本地专属扩展，并建立可测试映射。

## 3. 分层与唯一管理者

```text
应用 OperationScope
  ├─ rs-fs facade：逻辑路径、选项校验、copy fallback、领域错误
  ├─ provider：实际请求/session、预算、效果与释放事实
  │    └─ rs-fs-local：复用 LocalIoControl/ResourceId，不重复纳管
  └─ rs-operation：取消/期限、登记、result/released、资源依赖、恢复注册表
```

本库不增加通用 worker 池，不把同步 SPI 默认放进 async poll。同步-only provider 若要支持 async，必须通过其自己的受控执行器适配；缺失能力返回 Unsupported。纯 Rust 值校验保持同步，不产生服务线程。

FileSystem/AsyncFileSystem 持有 provider、配置快照和显式执行绑定。应用注入 supervisor 及 PortableDriver/ExternalDriver；具体 I/O 仍由 provider 驱动。不为每个 facade/每次调用创建 supervisor，不把外部运行时提前关闭的情况隐藏为成功释放。

## 4. 公共操作接口与 owned SPI

新增 execution 模块：FsExecutionBinding、FsOperationRequest、FsOperationOutcome、FsOperationFailure 和 FsRecoveryHandle。它们保留领域事实，生命周期 handle 复用 OperationHandle<T,E>。

目标 API 摘要（不是当前可编译示例）：

```rust,ignore
type FsOperation<T, E = FsOperationFailure> = OperationHandle<T, E>;
impl FileSystem {
    fn with_execution(self, binding: FsExecutionBinding) -> Self;
    fn submit_read_all_in(&self, scope: &OperationScope, path: Path,
        options: ReadAllOptions) -> Result<FsOperation<Vec<u8>>, FsSubmitFailure>;
    fn copy_managed_in(&self, scope: &OperationScope, source: &Path,
        target: &Path, options: CopyOptions) -> Result<CopyOutcome, CopyFailure>;
}
impl AsyncFileSystem {
    fn with_execution(self, binding: FsExecutionBinding) -> Self;
    async fn read_prefix_in(&self, scope: &OperationScope, path: &Path,
        options: ReadOptions, max_bytes: usize) -> FsResult<PrefixReadOutcome>;
    fn submit_copy_in(&self, scope: &OperationScope, source: Path,
        target: Path, options: CopyOptions) -> Result<FsOperation<CopyOutcome>, FsSubmitFailure>;
}
```

owned submit 的 path、options、provider Arc 与输入流均可移动到后台，先 reserve 公共槽和 provider 容量，再 attach control、commit。FsSubmitFailure 归还未启动 owned request，并保留 AdmissionError/提供方失败；不得在返回失败后实际启动远程修改。

0.4 SPI 增加统一 owned dispatch prepare 协议：先构造无副作用 PreparedFsOperation，包含 ManagedOperationControl 与提交许可；facade 完成 attach 后 commit，才允许发出改变远程对象或创建原生资源的请求。同步和异步 provider 都必须满足这个管理协议；具体准备方法可以同步返回待驱动 future，但不能在未 attach 前开始 I/O。

现有 object-safe typed operation 方法可保留为 provider 内部执行方法，由 prepared dispatch 在纳管后调用；公开“托管”能力不得绕过门禁直接调用它们。0.3 provider 不符合协议时明确报 ManagedLifecycleUnsupported，不能先做操作再登记。迁移期普通旧同步入口可保留，但文档不得宣称具备托管保证。

借用便利方法只在前台保留借用；跨 future Drop 继续执行的工作必须拥有输入/session，读取 owned 有限缓冲后交回借用方。禁止延长请求引用生存期，禁止把非 Send provider future 移交 PortableDriver。特定运行时 future 使用 ExternalDriver 并持有其 lease。

## 5. 与 local-files 的能力对齐矩阵

表中“新增”是 0.4 设计目标，不声称 0.3 已有。同名语义按 guarantee/request/outcome 三层表达；provider 可明确拒绝。

| local-files 能力 | rs-fs 0.4 对应 | 保证和 provider 边界 |
| --- | --- | --- |
| Host/Rooted authority | 保留 provider instance 与路径约束 | 逻辑路径不是宿主授权路径，不把 Rooted 转 Host |
| metadata/同句柄观测 | metadata、OpenedFileInfo、ReadObservation | 明确是打开时/读前/读后观测，非原子快照 |
| read_prefix/read_all/read_text | 有限 ReadAllOptions 和完整读取接口 | 默认有限；预算超限不返回截断内容冒充完整 |
| reader/随机读取 | FileReader/RandomAccessReader 及 async 对等 | 同一打开对象或不可变版本；禁止按路径重开 |
| consistent/snapshot | 新 ReadConsistencyPolicy/ConsistentReadOutcome | 保留 observed-change 与内容快照的区别 |
| writer commit/abort | 现有 writer 加公共资源登记 | 发布事实、持久性和 cleanup 分别报告 |
| create-new/append/replace | WriteDisposition/WriteOptions | append 不承诺整次原子，取消不回滚已追加字节 |
| digest 条件写入 | 新 ContentPrecondition/conditional writer | 校验与发布必须同一协议，不能先读 hash 再无条件覆盖 |
| copy/source-mode/conflict | CopyOptions 增加源观测与策略 | native/fallback 路线、计数、部分结果均保留 |
| rename | RenameOptions/Outcome/Failure | 不支持跨 provider 原子 rename，不静默退化复制删除 |
| walker/递归限制 | ListOptions/AsyncDirectoryStream | 条目/深度/字节/时间预算，不全量物化 |
| create/delete | 既有 outcome + effect/context | 部分目录已删除必须报告 PartiallyApplied |
| temp private/cleanup/keep/persist | 扩展 TempOptions/资源状态 | 创建时隐私保证，不以创建后 chmod 冒充 |
| seal/只读临时资源 | 新 SealedTempFile/AsyncSealedTempFile | 关闭本 owner 写能力，不默认宣称全局不可变 |
| staged-tree/validate/publish | 新 StagedDirectory/TreePublishOptions | 本地与层级 provider 可实现；对象前缀不可伪造原子树发布 |
| security/access observation | 通用只声明标准化访问要求 | ACL/UID/GID/Windows ACE 仍走类型化 provider 扩展 |
| filesystem space/limits | FileSystemLimits 和新 StorageObservation | 可未知，观测容量不保证后续一定写成功 |
| 原生文件导出 | rs-fs-local 显式本地扩展 | 核心不出现 native_path/File；导出仍持 ResourceUse |

本版必须为这些类别定义类型、同步/异步 facade 对应、SPI 能力及 conformance 用例。不是所有 provider 必须支持每项；无法表达语义的 provider 显式 Unsupported，而不是丢选项后执行低保证操作。

## 6. 能力、要求和运行时事实

扩展 ProviderOperation 以覆盖 ReadConsistent/OpenConditionalWriter/SealTemp/CreateStagedDirectory/ValidateTree/PublishTree/ObserveStorage 等入口；细粒度操作通过各 session SPI 表达，保持 object-safe。FileSystemCapability 增加相应语义保证，ProviderProperties 校验每个 Guaranteed 都有实际原语支撑，async capability 单独核对不能从 sync 自动继承。

必须区分：支持调用、对某请求可尝试、这次实际达到的保证。Conditional 表示运行时可能失败，不能等价 Guaranteed。Required 保证已知不支持时副作用前失败；Preferred 可以降级，但必须报告 achieved guarantee；运行中才发现能力不足则报告真实副作用，不能伪装早期拒绝。

本地原子性/持久性限制完整传递，包括 Windows 不支持的模式、目录 fsync/rename 限制等；不从 provider 名称推断保证。对象存储一致性、版本和不可变对象能力按具体 provider 声明；ETag 不默认等价内容 hash 或强版本。

能力查询本身无 I/O；需要探测时显式 observe/query 操作，结果可能过时。文件系统剩余空间、权限和符号链接状态不作为后续操作一定成功的证明。

## 7. 读取、快照与随机访问

新增 ReadAllOptions 明确 max_bytes，默认 16 MiB；既有 prefix 接口保持调用方指定上限。用上限加一哨兵区分“确实到 EOF”和“被截断”，内容长度/分配检查使用 checked arithmetic。普通大文件用流，不建议提高全局默认预算。

ReadConsistencyPolicy 建议包括 BestEffort、ObserveChanges、RequireVersion。ConsistentReadOutcome 返回有限 bytes、打开对象身份、前后观测、是否变化/未知；只有 provider 明确提供 immutable-version/snapshot 保证才能报告强一致快照。ObserveChanges 不承诺检测所有并发修改，不用 file length/mtime 伪造强版本。

RandomAccessReader 绑定一次打开对象或版本；read_at 计入累计预算并遵守 provider max_segment。取消后未完成读取继续持有 ResourceUse；流 Pending 时不能让 worker 写入已经失效的借用缓冲。使用 owned 内部缓冲、串行化偏移或有能力的 offset I/O，保持与 local-files 同样的取消安全原则。

读失败保留实际已读取/采样计数。部分 prefix 可作为明确的 PrefixReadOutcome，但完整读取超限不能返回 Ok(bytes) 隐藏截断。Magika 只需要完整逻辑长度和有限同对象采样，不应被迫 read_all。

## 8. 条件写入、发布与恢复

保留 WritePrecondition::IfAbsent/IfMatch，新增内容条件协议与版本条件区分。ContentPrecondition 包含算法和摘要，provider 必须保证比较与发布在其有效一致性协议中完成；若本地条件 writer 只承诺某类并发模型，映射后不能升级成任意并发写都线性化的 CAS。

writer 状态至少覆盖 Open、Writing、Committing、Published、Aborting、Aborted、RecoveryRequired。所有异步资源归 scope 登记；业务访问 handle 的 Drop 不直接做阻塞 abort。commit 前取消可请求 abort；commit 越过发布线性化点后取得真实结果，再根据状态报告取消伴随事实。

FsEffectState 保持 Unchanged/PartiallyApplied/Applied/Indeterminate，与已达到原子性、数据同步、命名空间持久性和 cleanup 事实分别呈现。发布成功后 fsync 失败仍是 Applied；远程 timeout 无法知道提交是否生效则 Indeterminate。框架不会把这些事实改成统一 Cancelled。

恢复使用公共 RecoveryKey/注册表保存 provider-owned recovery，FsRecoveryHandle 提供领域 retry_abort/keep/inspect 操作；不公开任意 Any downcast。原生 LocalFileError source/typed recovery 在 local adapter 中保留；一般调用方可通过通用 handle 继续处理而不依赖本地类型。

## 9. 临时资源、seal 与跨域使用

TempOptions 增加 PrivacyRequirement、创建尝试上限、清理预算和必要容量；通用默认不假设存在宿主 temp_dir。provider 选择合法临时区，返回结果必须满足逻辑路径/前缀约束和声明的隐私要求。

临时资源创建前预留 scope resource slot，成功后 owner 归公共 registry，访问 handle 持租约。keep/persist 成功后通过公共所有权转交完成，不让原 temp cleanup 删除已发布对象；persist 部分失败明确源保留/目标发布/cleanup 状态并保存恢复 owner。

seal 关闭本 owner 的写入接口并保留只读访问/观测；同步与 namespace 持久化要求可分别配置。没有全局不可变保证时不称 immutable。普通 rs-fs SealedTempFile 提供逻辑读取，不提供 native_path；外部 file/ffprobe 必须经本地授权适配器或另建 Host 暂存。

多使用者都持 ResourceUse；清理在最后一个真实使用者释放后运行。命令进程退出但输入 helper 尚未退出时，不允许 local adapter 归还资源使用租约。临时对象 remote effect 不确定时保留恢复信息，不因本地请求 future 被丢弃而声称远程已删除。

## 10. 目录树、复制与遍历

新增 StagedDirectory/AsyncStagedDirectory 与 TreeValidationOptions/TreePublishOutcome，提升 local-files 已有的暂存树、精确条目验证、发布和恢复语义。验证规则包括期望条目、链接/特殊文件策略、条目数/深度/读取字节/时间预算；校验失败不等于 staging 不存在，必须保留 cleanup owner。

TreePublishOptions 分开请求原子性、已有目标冲突策略、元数据保留、文件内容同步与命名空间持久性。实现不了树整体发布的对象存储返回 Unsupported/Conditional；不能用上传 manifest 冒充所有 reader 都会遵循的原子目录切换。树访问权限仅采用可移植访问要求，本地 ACL 走本地扩展。

copy 先评估 native TryCopy，只有已声明允许的情况才走流 fallback。native 已有副作用后不得偷偷重试另一条路线；流 fallback 通过同 scope 的 reader、writer 和 shared budget 组合，发生发布/abort 失败都保留 outcome/recovery。远程服务器任务由 provider control 管理，不能仅因 HTTP 连接关闭就宣布任务终结。

目录遍历默认按批次 128 条/256 KiB 记录预算，与 local-files 对齐，provider 可更严格；单条过大返回明确限制错误。逻辑对象前缀、层级路径、根目录和符号链接保持既有区别。递归 delete 仍不保证事务，报告确实已删除的对象/条目数量及未知部分。

## 11. 结果、释放与远程不确定性

FsOperation<T,E> 只复用公共 result/released/observer，不再给 AsyncCopyOperation/AsyncWriteAllOperation 增加第二套终结循环。可保留名称作为领域 builder/wrapper，execute 迁移到公共观察；恢复所有权从对象私有 slot 迁移到同一公共注册表，旧 take_recovery 通过强类型包装转发。

远程业务效果未知和本地资源释放是不同维度：请求连接/本地 session 已终结，但对象是否已写入未知时，可以有 Released + FsEffectState::Indeterminate；如果 provider 仍控制一个继续使用资源的远程 job，则需要保留 job control/租约，直到其真正终结或明确将责任转交恢复管理者，不能伪造 Released。

provider 必须声明其 ChildSettlement 覆盖的责任范围。只有“停止继续 poll”没有 release/transfer 证明的 provider 不具备完整托管契约。observer 超期仅提供快照，后续仍可观察或取恢复 handle；Drop 不自动补偿已发布对象。

## 12. 与 local-files/适配器的实施协同

rs-local-files 0.6 与本库 0.4 都先依赖已验收 operation 0.1，两者互不依赖。rs-fs-local 后续实现双向策略映射和 sync/async SPI，直接复用 LocalIoControl、resource ID 与 ResourceUse；facade 不能再包一个独立 root/context/token 把一次 I/O 变成两份生命周期。

要求为第 5 节矩阵每一项提交映射表：已有原语、通用选项、achieved guarantee、失败 effect、恢复 owner、平台限制和测试。每个 Required 项做正例与不支持例；本地内部类型不能被擦成 provider_metadata 字符串后声称完成对齐。

原生路径/句柄、安全观测等本地扩展由 rs-fs-local 提供显式类型化入口；取得原生能力需要应用授权，不破坏 Rooted 隔离语义。其他 provider 的扩展保持独立，不要求 core 依赖操作系统 crate。

本轮只新增本库设计，不修改 rs-fs-local 的源码或版本。适配器未验收前不得宣称全链本地异步可用；但本库必须先用假 provider 和适配原型证明设计可以保留这些语义。

## 13. 依赖与兼容迁移

目标 qubit-operation ^0.1 为必需基础依赖；qubit-io/qubit-budget 继续现有系列，实际兼容性在实现时验证。default=[] 保持；async feature 仅启用异步 facade/SPI，不改变核心共享状态类型，也不隐式创建服务。

0.4 不兼容点包括 owned prepare/commit SPI、context 注入、operation/recovery 观察入口和新增结果类型。所有 provider、registry 包和文档 fixture 需按依赖图迁移，不只改 rs-fs Cargo 版本。ProviderOperation/Capability 新值需更新验证矩阵、ALL 和序列化/匹配逻辑；已有 non_exhaustive 不代替实际迁移。

建议保留旧简单 read/stat 同步便利方法，明确直接执行不托管；新的 managed/async_mutation 必须走协议，禁止静默退回不托管路线。已有实现不得因为签名能编译就假定取消与恢复语义仍正确。

关联设计：[local-files 0.6](../../rs-local-files/doc/design-0.6.0.zh_CN.md)、[command 0.12](../../rs-command/doc/design-0.12.0.zh_CN.md)、[mime 0.22](../../rs-mime/doc/design-0.22.0.zh_CN.md)、[magika 0.19](../../rs-magika/doc/design-0.19.0.zh_CN.md)。

## 14. 实施顺序

1. 先实现并验收 rs-operation，再冻结 owned SPI 和 shared context/recovery 契约。
2. 完成共同 capability/requirement/outcome 类型及假 provider，建立第 5 节逐项映射测试。
3. 迁移 read/write/copy/temp/list/session 到公共生命周期，保持现有 typed failures。
4. 补齐一致性/条件内容写入/seal/staged-tree/观测的 sync/async 对等接口。
5. 在 rs-fs-local 适配验证后迁移其他 provider/registry，最后接 MIME/Magika 消费测试。

common 类型不能反向依赖 local-files；native 实现和 fs facade 可在 operation 验收后并行，但消费方必须等待共同契约稳定。

## 15. 验收标准

- 同步/异步对同一 provider 返回等价业务结果、effect、achieved guarantee 和恢复能力。
- reserve/attach/commit 与取消竞争不产生无主修改；队列满仍可通知停止/清理。
- required 能力不支持时不产生本可避免的副作用；运行时失败保留已发生事实。
- 同对象随机读取经过路径替换仍绑定原对象，旧版本/短读/未知长度/段限制与累计预算均覆盖。
- pending 读/写/seek future Drop 后无悬垂借用、字节重复、隐藏并发写；必要时 settle 后再用资源。
- 发布后取消、fsync 失败、远程结果未知、copy fallback 部分失败、临时 persist 失败均保留正确 effect/recovery。
- 多使用者、命令 helpers 未结束、原生 job 未确认期间不清理输入；恢复 owner 不因诊断溢出丢失。
- staged-tree 精确验证、符号链接/特殊条目、递归限制和不支持原子树发布的 provider 均有测试。
- Host/Rooted、非 UTF-8 本地路径和逻辑 provider 路径不互相冒充；原生扩展不能突破授权。
- Linux/Windows/现有支持平台及层级/对象 key 假 provider 全部进入矩阵；不支持行为也需断言。
- 文档/fixture/包构建、registry 能力发现和 mime/magika 下游测试通过，未运行或有意不支持的能力明确列出。

领域吞吐、线程数、内存、取消到真实释放延迟分别与 0.3 基线比较；同步和异步 API 对齐不等于自动提高底层存储性能。本次仅设计修订，不运行实现测试或宣称任何目标能力已经完成。
