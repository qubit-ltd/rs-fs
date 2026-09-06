# Migrating to qubit-fs 0.3

Version 0.3 makes operation recovery facts explicit. The provider-neutral facade keeps the existing synchronous APIs and the legacy asynchronous `write_all`, while adding an owning asynchronous write operation for callers that must survive cancellation.

## Effect certainty and temporary resources

Use `FsError::has_indeterminate_effect()` whenever recovery depends on whether a provider-side action took effect. It returns true for both `FsErrorKind::Indeterminate` and an `FsEffectState::Indeterminate` attached to another error kind.

`PersistFailureState` now distinguishes source ownership:

```rust
match failure.state() {
    PersistFailureState::NotPublished => { /* the handle still owns cleanup */ }
    PersistFailureState::NotPublishedSourceReleased => { /* the source is already released */ }
    PersistFailureState::PublishedSourceRetained => { /* target exists; clean the source explicitly */ }
    PersistFailureState::PublishedSourceReleased => { /* target exists and the handle is terminal */ }
    PersistFailureState::Indeterminate => { /* inspect the provider before retrying */ }
}
```

After publication, `PersistFailure::publication_target()` reports the target that was successfully published, even when a later request names another target.

## Owning asynchronous writes

Prefer `begin_write_all` when the caller may be cancelled between open, write, flush, and commit:

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

`AsyncWriteAllOperationFailure` carries the state and confirmed byte count. If a cancellation leaves a recovery writer, abort it explicitly and retain any abort error alongside the primary failure. The deprecated `AsyncFileSystem::write_all` remains as a compatibility wrapper; callers that need a recovery handle should migrate to the owning operation.

## Listing filters

`ListOptions::with_prefix` retains the hierarchical subtree behavior. Use `ListOptions::with_filter(ListFilter::LiteralPrefix(raw))` with `ListOptions::object_keys()` for flat object namespaces. Literal prefixes are matched as raw key text; they are not decoded or normalized, and a non-empty root is required. Providers may reject a literal request when their SDK cannot preserve the requested key representation.

`Subtree("logs")` matches `logs` and descendants under `logs/`; it does not match `logs-old`. `LiteralPrefix("logs")` matches raw keys beginning with `logs`, including `logs-old`.

## Downstream versions

The compatible sibling releases are `qubit-fs` 0.3, `qubit-fs-local` 0.2, `qubit-fs-registry` 0.2, `qubit-fs-testkit` 0.2, and `qubit-mime` 0.11. `qubit-magika` keeps its package version and updates its `qubit-mime` dependency to 0.11.
