# Coverage exemption review (0.4)

[简体中文](coverage_review.zh_CN.md)

2026-09-07

This change reduces the original 56 exemptions to 39 by removing 17. It adds no exemptions, lowers no thresholds, and introduces no coverage-dependent production paths. Retained entries are recorded testing debt, not evidence that the code is untestable or verified correct. Add behavioral assertions, validate both default and all-features gates, then remove exemptions.

## Measurement scope

Measured locally on Linux. Stable llvm-cov gates require 95% functions, 90% lines, and 85% regions after configured file exclusions. The full-source figures below include exemptions and are not gate results. These snapshots follow the core regression additions; subsequent Rustdoc additions are not included in these historical measurements.

| Mode | Functions | Lines | Regions |
| --- | --- | --- | --- |
| default | 716/752 (95.21%) | 4334/4668 (92.84%) | 5527/6022 (91.78%) |
| all | 907/1020 (88.92%) | 6029/6783 (88.88%) | 7936/9045 (87.74%) |

Actual branch measurement uses `nightly-2026-06-05` with `cargo llvm-cov --branch`, passing `--no-default-features` and `--all-features` respectively. Do not interpret differences between toolchains as improvements or regressions. Zero reported stable branches is not full branch coverage.

| Mode | Branches |
| --- | --- |
| default | 490/598 (81.94%) |
| all | 660/806 (81.89%) |

## 17 removed entries

| File | Reason |
| --- | --- |
| `src/copy/async_copy_failure.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/write/async_write_all_failure.rs` | Obsolete async failure type removed. |
| `src/metadata/resource_version.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/spi/spi_rename_failure.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/copy/internal/copy_failure_parts.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/copy/internal/copy_recovery_snapshot.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/directory/internal/list_selection.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/directory/list_filter.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/temp/internal/temp_lifecycle.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/write/async_write_all_operation.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/write/async_write_all_operation_failure.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/write/async_write_all_operation_state.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/write/internal/write_all_cancellation_guard.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/write/internal/write_all_recovery_snapshot.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/directory/directory_entry_validation.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/temp/persist_failure.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |
| `src/temp/persist_failure_state.rs` | Functions covered or declaration-only; no longer exempt the entire file from the gate. |

## 39 retained entries

The table uses the stable all-features snapshot; numerators are actual covered counts. Low coverage alone does not justify permanent exemption. The last column identifies the testing work needed before removal. Prioritize copy fallback, writers, and async temporary-resource behavior combinations, then utility access.

| File | Functions | Lines | Reason and follow-up |
| --- | --- | --- | --- |
| `src/async_file_system.rs` | 66/71 | 453/478 | Async facade provider rejection, temporary-resource validation, and error enrichment still have gaps. |
| `src/copy/async_copy_operation.rs` | 20/25 | 335/460 | Native-copy/fallback failure combinations and recovery access do not yet have a complete matrix. |
| `src/copy/copy_failure.rs` | 8/12 | 47/59 | Recovery access, consuming conversions, and formatting still contain unexecuted functions. |
| `src/write/async_file_writer.rs` | 21/25 | 215/253 | Writer terminal states, invalid progress, and cleanup branches are not all exercised. |
| `src/write/file_writer.rs` | 17/20 | 193/229 | Synchronous writer invalid progress, repeated termination, and cleanup still have gaps. |
| `src/write/write_all_failure.rs` | 7/8 | 28/31 | The synchronous failure carrier still has unexecuted access or consuming APIs. |
| `src/metadata/non_sensitive_metadata.rs` | 7/9 | 21/27 | Metadata-container utility accessors are not all exercised. |
| `src/metadata/user_metadata.rs` | 6/8 | 27/33 | User-metadata container utility APIs are not all exercised. |
| `src/metadata/write_outcome.rs` | 9/11 | 38/44 | Write-outcome construction and fact-access combinations are not all exercised. |
| `src/copy/copy_outcome.rs` | 14/22 | 97/118 | Copy-outcome guarantee validation combinations and fact access still have gaps. |
| `src/copy/copy_operation.rs` | 21/25 | 310/396 | Synchronous native-copy/fallback error combinations and recovery branches remain incomplete. |
| `src/copy/internal/fallback_failure_state.rs` | 3/4 | 25/39 | Fallback failure-state mapping has an unexecuted function; prioritize state-combination tests. |
| `src/directory/create_directory_options.rs` | 4/7 | 18/28 | Directory-creation option construction, access, and validation are not all exercised. |
| `src/directory/create_directory_outcome.rs` | 3/4 | 13/16 | The stable-toolchain report still has one uncovered outcome accessor. |
| `src/directory/delete_outcome.rs` | 3/4 | 13/16 | The stable-toolchain report still has one uncovered deletion-outcome accessor. |
| `src/read/read_options.rs` | 13/14 | 69/72 | Read options still have one uncovered utility API. |
| `src/write/write_options.rs` | 18/20 | 115/121 | Write options still have uncovered utility APIs. |
| `src/path/path.rs` | 16/18 | 114/123 | Path conversions and convenience APIs still have unexecuted paths. |
| `src/spi/internal/resolved_options.rs` | 1/2 | 3/6 | The resolved-option container consuming API is uncovered. |
| `src/spi/resolved_copy_options.rs` | 2/3 | 9/12 | Resolved-copy option utility access is uncovered. |
| `src/spi/resolved_list_options.rs` | 1/3 | 6/12 | Resolved-list option access/consumption APIs are not all covered. |
| `src/temp/async_temp_directory.rs` | 5/8 | 15/28 | Async temporary-directory access wrappers and lifecycle delegation are not all exercised. |
| `src/temp/persist_outcome.rs` | 6/8 | 26/32 | Persistence-outcome utility fact access is not all exercised. |
| `src/temp/temp_directory.rs` | 12/16 | 128/145 | Temporary-directory invalid lifecycle and path convenience APIs still have gaps. |
| `src/temp/temp_options.rs` | 8/10 | 33/39 | Temporary-resource option construction and utility access are not all exercised. |
| `src/temp/temp_file.rs` | 12/14 | 129/139 | Temporary-file path APIs and invalid lifecycle paths are not all exercised. |
| `src/uri/connection_uri.rs` | 9/10 | 41/44 | The stable-toolchain report still has one uncovered URI utility API. |
| `src/uri/uri.rs` | 12/16 | 47/60 | URI conversion, credential redaction, and utility access still have unexecuted APIs. |
| `src/directory/async_directory_operation.rs` | 4/5 | 41/42 | Async listing still has an unexecuted function; high line coverage does not establish full function coverage. |
| `src/directory/async_directory_stream.rs` | 11/15 | 95/106 | Async stream access, termination, and invalid provider-entry handling still have gaps. |
| `src/directory/directory_operation.rs` | 4/5 | 39/40 | Synchronous listing still has an unexecuted function and needs applicable option combinations. |
| `src/directory/directory_stream.rs` | 11/12 | 88/96 | Synchronous directory-stream termination and utility APIs still have unexecuted paths. |
| `src/directory/list_options.rs` | 24/27 | 120/180 | Filter, depth, and resource-limit option combinations have substantial coverage gaps. |
| `src/error/fs_error.rs` | 28/30 | 164/171 | Error utility conversion and formatting still have unexecuted APIs. |
| `src/facade/facade_core.rs` | 15/16 | 85/94 | Facade resource-limit and provider-contract error paths are not all exercised. |
| `src/file_system.rs` | 44/50 | 336/356 | Synchronous facade temporary-resource, deletion, and rename rejection paths still have gaps. |
| `src/temp/async_temp_file.rs` | 18/22 | 174/197 | Async temporary-resource utility path APIs and lifecycle failure combinations are not all exercised. |
| `src/metadata/symlink_policy.rs` | 0/1 | 0/3 | Stable records no execution of follows; nightly covers it, but that does not prove the stable gap is an instrumentation artifact. |
| `src/path_constraints.rs` | 4/5 | 22/27 | Either path-form and convenience-constructor combinations are not all exercised. |
