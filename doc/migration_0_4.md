# Migrating to qubit-fs 0.4

[中文](migration_0_4.zh_CN.md) · [User guide](user_guide.md)

Version 0.4 intentionally removes compatibility with the old async whole-file
write entry point. Synchronous `FileSystem::write_all` remains available.

## Application changes

- Replace async `write_all` with `begin_write_all(path, bytes, options)` followed
  by `operation.execute().await`. Keep the operation outside cancellation scopes.
- Pass an owned `Vec<u8>`. Use `to_vec()` explicitly when starting from borrowed
  data, or use `open_writer` for streaming. The operation no longer borrows the
  filesystem or payload and has no lifetime parameter.
- Replace `AsyncWriteAllFailure` with `AsyncWriteAllOperationFailure`; the
  operation holds the recovery writer, while the failure holds error/state/count.
- Successful execution now records all acknowledged bytes and releases the
  completed writer. `has_recovery_writer()` is false after success.
- Treat operation state and byte count as historical facts. Taking and recovering
  the writer does not rewrite those facts. Repeated execution returns
  `InvalidState` without invoking the provider again.
- Inspect `filesystem()` and `path()` if an indeterminate opening has no writer.
  Do not interpret one absent `stat` result as proof of remote completion.

The [complete recovery examples](user_guide.md#async-writing-and-cancellation)
retain the primary error, any cleanup error, and the operation. No legacy
wrapper or public alias is retained.

## Provider changes

An opening failure must explicitly report `FsEffectState::Unchanged` only when
there are no external effects or residual cleanup obligations. An indeterminate
error kind overrides a conflicting unchanged annotation. Missing effect,
`Applied`, `PartiallyApplied`, and indeterminate opening all map to whole-operation
`Indeterminate`: opening a staging resource does not publish the requested file.
`AlreadyExists + Skip` requires explicit unchanged evidence.

Default unsupported SPI opens declare unchanged because they perform no I/O.
Adapters may annotate pure preflight failures but must preserve uncertainty from
native I/O. Commit and cleanup keep their own stage-specific states.

## Contract fixtures

Async write fixtures must implement `prepare_write_cancellation` with real
open/write/flush/commit gates. Unsupported probes for an advertised write
capability produce `Unverified` and fail strict report completeness. Synchronous
suites do not register these async obligations. Request-only legacy fixtures are
not evidence that a cancellation point was reached.

## Coordinated dependencies

| Package | Version |
| --- | --- |
| `qubit-fs` | 0.4 |
| `qubit-fs-local`, `qubit-fs-registry`, `qubit-fs-testkit` | 0.3 |
| `qubit-mime` | 0.12 |
| `qubit-magika` | 0.10 |

Update sibling dependencies together where public types cross crate boundaries.
No MIME detection algorithm changes are required. The [0.3 migration notes](migration_0_3.md)
remain historical guidance for that release, not the current API.
