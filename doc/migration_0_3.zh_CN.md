# qubit-fs 0.3 迁移指南

0.3 版把操作恢复事实明确暴露给调用方。同步 API 和旧的异步 `write_all` 保持兼容，同时新增一个拥有请求数据和恢复句柄的异步整文件写入操作，用于必须安全处理取消的调用方。

## effect 确定性与临时资源

当恢复决策取决于 provider 是否已经产生副作用时，使用 `FsError::has_indeterminate_effect()`。它会同时识别 `FsErrorKind::Indeterminate` 和附着在其他错误 kind 上的 `FsEffectState::Indeterminate`。

`PersistFailureState` 现在区分 source 是否仍由 handle 负责：

```rust
match failure.state() {
    PersistFailureState::NotPublished => { /* handle 仍负责清理 source */ }
    PersistFailureState::NotPublishedSourceReleased => { /* source 已释放 */ }
    PersistFailureState::PublishedSourceRetained => { /* target 已发布，显式清理 source */ }
    PersistFailureState::PublishedSourceReleased => { /* target 已发布且 handle 已终态 */ }
    PersistFailureState::Indeterminate => { /* 先核查 provider，再决定是否重试 */ }
}
```

发布成功后，即使后续请求使用另一个 target，`PersistFailure::publication_target()` 仍返回实际已发布的 target。

## 拥有请求的异步写入

如果调用方可能在 open、write、flush、commit 之间被取消，优先使用 `begin_write_all`：

```rust
let mut operation = filesystem.begin_write_all(path.clone(), bytes, options)?;
match operation.execute().await {
    Ok(outcome) => println!("published {} bytes", outcome.bytes_written().unwrap_or(0)),
    Err(failure) => {
        if let Some(mut writer) = operation.take_recovery_writer() {
            let _ = writer.abort_async().await;
        }
        return Err(failure.into_error());
    }
}
```

`AsyncWriteAllOperationFailure` 携带状态和已确认的字节数。取消后如果 operation 保留 recovery writer，应显式 abort，并保留主错误与 abort 错误。旧的 `AsyncFileSystem::write_all` 仍作为兼容包装存在；需要恢复句柄时应迁移到拥有 operation 的入口。

## listing filter

`ListOptions::with_prefix` 继续表示层级子树。扁平对象命名空间使用 `ListOptions::object_keys()` 配合 `ListOptions::with_filter(ListFilter::LiteralPrefix(raw))`。literal 前缀按原始 key 文本匹配，不解码、不归一化，并要求非空 root。底层 SDK 无法保留 key 表示时，provider 应拒绝请求。

`Subtree("logs")` 匹配 `logs` 及 `logs/` 下的后代，不匹配 `logs-old`。`LiteralPrefix("logs")` 匹配以 `logs` 开头的原始 key，包括 `logs-old`。

## 下游版本

兼容的 sibling 版本为：`qubit-fs` 0.3、`qubit-fs-local` 0.2、`qubit-fs-registry` 0.2、`qubit-fs-testkit` 0.2、`qubit-mime` 0.11。`qubit-magika` 的 package 版本保持不变，只把 `qubit-mime` 依赖更新到 0.11。
