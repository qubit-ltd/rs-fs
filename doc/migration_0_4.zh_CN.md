# 迁移到 qubit-fs 0.4

[English](migration_0_4.md) · [用户指南](user_guide.zh_CN.md)

0.4 删除旧异步整文件写入入口，不保留兼容包装。同步 `FileSystem::write_all` 继续保留。

## 应用迁移

- 将异步 `write_all` 改为 `begin_write_all(path, bytes, options)`，再执行
  `operation.execute().await`；operation 必须位于取消作用域外。
- 传入拥有所有权的 `Vec<u8>`。借用数据显式 `to_vec()`，大数据流使用 `open_writer`。
  operation 不再借用文件系统或数据，类型不再有生命周期参数。
- 用 `AsyncWriteAllOperationFailure` 替代 `AsyncWriteAllFailure`。错误对象保存主错误、
  状态和计数；operation 保存恢复 writer。
- 成功执行会记录全部已确认字节，并释放已完成的 writer；此时 `has_recovery_writer()` 为 false。
- operation 的状态和计数是历史事实。取走 writer 后的恢复不会改写历史；重复执行返回
  `InvalidState`，不会再次调用提供者。
- 打开结果不确定且没有 writer 时，通过 `filesystem()` 和 `path()` 核查，不能用一次
  `stat` 未找到目标作为远端操作已经结束的证明。

[完整恢复示例](user_guide.zh_CN.md#异步写入与取消)保留主错误、清理错误和 operation。
旧入口及公共别名均不保留。

## 提供者迁移

打开失败只有在没有外部副作用、也没有残留清理责任时，才能显式标记
`FsEffectState::Unchanged`。不确定错误类型优先于矛盾的 unchanged 注记。
缺少 effect、`Applied`、`PartiallyApplied` 或不确定打开结果，都使整次操作归为
`Indeterminate`：创建暂存资源不等于请求文件已经发布。
`AlreadyExists + Skip` 也必须有明确的 unchanged 证据。

默认不支持的 SPI 打开实现不执行 I/O，因此明确报告 unchanged。适配器可以注记纯本地校验
失败，但必须保留原生 I/O 的不确定性。提交和清理继续使用各自阶段的状态。

## 契约 fixture

异步写入 fixture 应通过 `prepare_write_cancellation` 提供真实的 open/write/flush/commit
阶段 gate。声明支持写入却不提供探针时，报告为 `Unverified`，严格完整性检查失败。
同步 suite 不登记这些异步要求。只有请求参数的旧 fixture 不能证明已经到达取消点。

## 协调依赖

| 包 | 版本 |
| --- | --- |
| `qubit-fs` | 0.4 |
| `qubit-fs-local` | 0.4 |
| `qubit-fs-registry`、`qubit-fs-testkit` | 0.3 |
| `qubit-mime` | 0.12 |
| `qubit-magika` | 0.10 |

公共类型跨 crate 传递时，应一起更新 sibling 依赖。无需修改 MIME 检测算法。
[0.3 迁移说明](migration_0_3.zh_CN.md)作为历史资料保留，不描述当前版本 API。
