# 覆盖率豁免复核（0.4）

[English](coverage_review.md)

2026-09-07

## 加固后的测量（2026-09-07）

测量基于加固提交 `a1a0b49`，环境为 Linux；稳定测量使用 `cargo llvm-cov 0.8.6`（Cargo 工具链 `1.94.0`），分支测量使用 `nightly-2026-06-05`。以下为不使用任何豁免的全源码报告；配置中的豁免未用于这些总数。

| 模式 | 函数 | 行 | Region | 分支（nightly） |
| --- | --- | --- | --- | --- |
| default（`--no-default-features`） | 718/753（95.35%） | 4438/4786（92.73%） | 5564/6051（91.95%） | 493/606（81.35%） |
| all（`--all-features`） | 916/1020（89.80%） | 6211/6950（89.37%） | 8009/9070（88.30%） | 665/814（81.70%） |

本轮行为断言覆盖了流式复制耐久性、能力强度校验，以及文件模式复制统计校验。受影响的原有豁免文件并未全部达到稳定豁免删除门槛（函数 >=95%、行 >90%、Region >85%）：`src/copy/async_copy_operation.rs` 为 21/25、383/511、489/685；`src/copy/copy_operation.rs` 为 21/25、373/476、487/630；`src/copy/copy_outcome.rs` 为 14/22、113/135、116/143。因此未删除任何豁免。`src/copy/internal/stream_copy_policy.rs`（7/7、52/52、57/57）和 `src/metadata/file_system_limits.rs`（21/21、109/110、119/121）并不在配置的豁免中，无需修改配置。

下文历史的 56 项降至 39 项快照保持不变。本轮测量是新增快照，不覆盖历史数据。

本次将原 56 项豁免减少为 39 项，删除 17 项，没有新增豁免、降低阈值或引入 coverage 条件生产路径。保留项是明确记录的测试债务，不代表无法测试或已经验证正确。后续应先补行为断言，再在默认和 all-features 门槛下验证后删除豁免。

## 测量口径

Linux 本地实际测量；stable llvm-cov 的门槛为函数 95%、行 90%、region 85%，统计时排除配置中的文件。以下全源码数值包含豁免，不能当作排除后的门槛结果。覆盖率快照在核心回归补齐后取得；后续 Rustdoc 增补不纳入这些历史测量值。

| Mode | Functions | Lines | Regions |
| --- | --- | --- | --- |
| default | 716/752 (95.21%) | 4334/4668 (92.84%) | 5527/6022 (91.78%) |
| all | 907/1020 (88.92%) | 6029/6783 (88.88%) | 7936/9045 (87.74%) |

真实分支测量另用 `nightly-2026-06-05` 与 `cargo llvm-cov --branch`，默认和 all-features 分别使用 `--no-default-features`、`--all-features`。不同工具链的数字不直接作增减对比；stable 中零个 branch 不是分支全覆盖。

| Mode | Branches |
| --- | --- |
| default | 490/598 (81.94%) |
| all | 660/806 (81.89%) |

## 删除的 17 项

| File | 理由 |
| --- | --- |
| `src/copy/async_copy_failure.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/write/async_write_all_failure.rs` | 旧异步失败类型已删除。 |
| `src/metadata/resource_version.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/spi/spi_rename_failure.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/copy/internal/copy_failure_parts.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/copy/internal/copy_recovery_snapshot.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/directory/internal/list_selection.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/directory/list_filter.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/temp/internal/temp_lifecycle.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/write/async_write_all_operation.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/write/async_write_all_operation_failure.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/write/async_write_all_operation_state.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/write/internal/write_all_cancellation_guard.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/write/internal/write_all_recovery_snapshot.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/directory/directory_entry_validation.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/temp/persist_failure.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |
| `src/temp/persist_failure_state.rs` | 函数已覆盖，或仅声明类型；不再保留整个文件的门槛豁免。 |

## 保留的 39 项

下表为 stable all-features 快照；函数/行分子均为实际覆盖数。低覆盖本身不是长期豁免的理由，最后一列明确了撤销前的补测方向。优先处理复制回退、writer 和异步临时资源的行为组合，再处理辅助访问接口。

| File | Functions | Lines | 保留原因与补测方向 |
| --- | --- | --- | --- |
| `src/async_file_system.rs` | 66/71 | 453/478 | 异步门面的 provider 拒绝、临时资源校验及错误补充仍有缺口。 |
| `src/copy/async_copy_operation.rs` | 20/25 | 335/460 | 原生复制与流式回退的错误组合、恢复访问仍未覆盖完整矩阵。 |
| `src/copy/copy_failure.rs` | 8/12 | 47/59 | 恢复事实的访问、消费及格式化接口仍有未执行函数。 |
| `src/write/async_file_writer.rs` | 21/25 | 215/253 | 写会话终态、非法进度及清理分支尚未全部执行。 |
| `src/write/file_writer.rs` | 17/20 | 193/229 | 同步写会话的非法进度、重复终止及清理分支仍有缺口。 |
| `src/write/write_all_failure.rs` | 7/8 | 28/31 | 同步失败载体仍有未执行的访问或消费接口。 |
| `src/metadata/non_sensitive_metadata.rs` | 7/9 | 21/27 | 元数据容器的辅助访问接口未全部执行。 |
| `src/metadata/user_metadata.rs` | 6/8 | 27/33 | 用户元数据容器辅助接口未全部执行。 |
| `src/metadata/write_outcome.rs` | 9/11 | 38/44 | 写结果的构造与事实访问组合尚未全部执行。 |
| `src/copy/copy_outcome.rs` | 14/22 | 97/118 | 复制结果的保证校验组合与事实访问尚有缺口。 |
| `src/copy/copy_operation.rs` | 21/25 | 310/396 | 同步原生复制/回退错误组合与恢复分支尚未完整覆盖。 |
| `src/copy/internal/fallback_failure_state.rs` | 3/4 | 25/39 | 回退失败状态映射尚有未执行函数；优先补状态组合测试。 |
| `src/directory/create_directory_options.rs` | 4/7 | 18/28 | 目录创建选项的构造、访问及验证路径未全部执行。 |
| `src/directory/create_directory_outcome.rs` | 3/4 | 13/16 | 稳定工具链报告仍有一个结果访问函数未覆盖。 |
| `src/directory/delete_outcome.rs` | 3/4 | 13/16 | 稳定工具链报告仍有一个删除结果访问函数未覆盖。 |
| `src/read/read_options.rs` | 13/14 | 69/72 | 读取选项仍有一个辅助接口未覆盖。 |
| `src/write/write_options.rs` | 18/20 | 115/121 | 写入选项仍有辅助接口未覆盖。 |
| `src/path/path.rs` | 16/18 | 114/123 | 路径转换与便捷接口尚有未执行路径。 |
| `src/spi/internal/resolved_options.rs` | 1/2 | 3/6 | 已验证选项容器的消费接口未覆盖。 |
| `src/spi/resolved_copy_options.rs` | 2/3 | 9/12 | 已验证复制选项的辅助访问接口未覆盖。 |
| `src/spi/resolved_list_options.rs` | 1/3 | 6/12 | 已验证列举选项的访问/消费接口未全部覆盖。 |
| `src/temp/async_temp_directory.rs` | 5/8 | 15/28 | 异步临时目录的包装访问和生命周期委托未全部执行。 |
| `src/temp/persist_outcome.rs` | 6/8 | 26/32 | 持久化结果的辅助事实访问未全部执行。 |
| `src/temp/temp_directory.rs` | 12/16 | 128/145 | 临时目录非法生命周期和路径便捷接口仍有缺口。 |
| `src/temp/temp_options.rs` | 8/10 | 33/39 | 临时资源选项的构造和辅助访问未全部执行。 |
| `src/temp/temp_file.rs` | 12/14 | 129/139 | 临时文件路径接口和非法生命周期路径未全部执行。 |
| `src/uri/connection_uri.rs` | 9/10 | 41/44 | 稳定工具链报告仍有一个 URI 辅助接口未覆盖。 |
| `src/uri/uri.rs` | 12/16 | 47/60 | URI 转换、凭据脱敏及辅助访问尚有未执行接口。 |
| `src/directory/async_directory_operation.rs` | 4/5 | 41/42 | 异步列举操作仍有未执行函数实例；不能把高行覆盖等同于函数全覆盖。 |
| `src/directory/async_directory_stream.rs` | 11/15 | 95/106 | 异步流访问、终止及无效 provider 条目处理仍有缺口。 |
| `src/directory/directory_operation.rs` | 4/5 | 39/40 | 同步列举操作仍有未执行函数，需补适用选项组合。 |
| `src/directory/directory_stream.rs` | 11/12 | 88/96 | 同步目录流终止和辅助接口尚有未执行路径。 |
| `src/directory/list_options.rs` | 24/27 | 120/180 | 过滤、深度和资源约束的选项组合明显未完整覆盖。 |
| `src/error/fs_error.rs` | 28/30 | 164/171 | 错误辅助转换和格式化仍有未执行接口。 |
| `src/facade/facade_core.rs` | 15/16 | 85/94 | 门面资源上限与 provider 契约错误路径未全部执行。 |
| `src/file_system.rs` | 44/50 | 336/356 | 同步门面临时资源、删除、重命名等拒绝路径尚有缺口。 |
| `src/temp/async_temp_file.rs` | 18/22 | 174/197 | 异步临时资源的辅助路径接口与生命周期错误组合未全部执行。 |
| `src/metadata/symlink_policy.rs` | 0/1 | 0/3 | 稳定工具链未记录 follows 的执行；nightly 有覆盖，不据此猜测稳定报告为伪差异。 |
| `src/path_constraints.rs` | 4/5 | 22/27 | Either 路径形式及便捷构造组合尚未全部执行。 |
