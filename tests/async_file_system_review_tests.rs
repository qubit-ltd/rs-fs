// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! External pending, failure, and rename coverage for the async facade.

#[path = "common/async_recording_spi.rs"]
mod async_recording_spi;
#[path = "common/poll_support.rs"]
mod poll_support;

use crate::async_recording_spi::{
    AsyncCopyStage,
    AsyncRecordingConfig,
    async_recording_file_system,
};
use crate::poll_support::{
    assert_pending,
    ready,
};
use qubit_fs::{
    AchievedAtomicity,
    AtomicityRequirement,
    CopyConflictPolicy,
    CopyFailureState,
    CopyOptions,
    CreateDirectoryOptions,
    DeleteOptions,
    DurabilityRequirement,
    FileKind,
    FileSystemCapability,
    FsErrorKind,
    ListOptions,
    Path,
    PathSemantics,
    PersistOptions,
    ReadOptions,
    RenameOptions,
    WriteOptions,
};

/// Parses one stable test path.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

/// Covers provider pending and failure propagation for facade I/O entry points.
#[test]
fn test_async_facade_stat_and_open_pending_and_error() {
    for stage in [
        AsyncCopyStage::Stat,
        AsyncCopyStage::OpenReader,
        AsyncCopyStage::OpenWriter,
    ] {
        let (fs, _) = async_recording_file_system(AsyncRecordingConfig {
            pending_stage: Some(stage),
            ..AsyncRecordingConfig::default()
        });
        match stage {
            AsyncCopyStage::Stat => {
                assert_pending(Box::pin(fs.stat(&path("/file"))).as_mut())
            }
            AsyncCopyStage::OpenReader => assert_pending(
                Box::pin(
                    fs.open_reader(&path("/file"), ReadOptions::default()),
                )
                .as_mut(),
            ),
            AsyncCopyStage::OpenWriter => assert_pending(
                Box::pin(
                    fs.open_writer(&path("/file"), WriteOptions::default()),
                )
                .as_mut(),
            ),
            _ => unreachable!(),
        }
        let (fs, _) = async_recording_file_system(AsyncRecordingConfig {
            failing_stage: Some(stage),
            ..AsyncRecordingConfig::default()
        });
        let error = match stage {
            AsyncCopyStage::Stat => ready(fs.stat(&path("/file")))
                .expect_err("provider failure expected"),
            AsyncCopyStage::OpenReader => {
                let Err(error) = ready(
                    fs.open_reader(&path("/file"), ReadOptions::default()),
                ) else {
                    panic!("provider failure expected");
                };
                error
            }
            AsyncCopyStage::OpenWriter => {
                let Err(error) = ready(
                    fs.open_writer(&path("/file"), WriteOptions::default()),
                ) else {
                    panic!("provider failure expected");
                };
                error
            }
            _ => unreachable!(),
        };
        assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    }
}

/// Covers rename preflight and result identity binding through the async
/// facade.
#[test]
fn test_async_rename_preflight_and_result_identity() {
    let (fs, probe) = async_recording_file_system(AsyncRecordingConfig {
        rename_atomicity: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    });
    let source = path("/source");
    let target = path("/target");
    let outcome = ready(fs.rename(&source, &target, RenameOptions::default()))
        .expect("rename should succeed");
    assert_eq!(&source, outcome.source());
    assert_eq!(&target, outcome.target());
    assert_eq!(vec!["rename"], probe.calls());
    let failure = ready(fs.rename(&source, &source, RenameOptions::default()))
        .expect_err("same path must fail locally");
    assert_eq!(FsErrorKind::InvalidOptions, failure.error().kind());
    assert_eq!(vec!["rename"], probe.calls());
}

/// Covers asynchronous facade contract failures and declined-copy boundary
/// failures that must remain typed and contextualized.
#[test]
fn test_async_facade_rejects_contract_and_fallback_boundary_failures() {
    let source = path("/source");
    let target = path("/target");

    for config in [
        AsyncRecordingConfig {
            rename_atomicity: Some(AchievedAtomicity::NonAtomic),
            ..AsyncRecordingConfig::default()
        },
        AsyncRecordingConfig {
            rename_atomicity: Some(AchievedAtomicity::Atomic),
            rename_copy_then_delete: true,
            ..AsyncRecordingConfig::default()
        },
    ] {
        let (file_system, _) = async_recording_file_system(config);
        let options = if file_system
            .properties()
            .capabilities()
            .contains(qubit_fs::FileSystemCapability::AtomicRename)
        {
            RenameOptions {
                atomicity: AtomicityRequirement::Required,
                ..RenameOptions::default()
            }
        } else {
            RenameOptions::default()
        };
        let error = ready(file_system.rename(&source, &target, options))
            .expect_err("invalid provider rename outcome must be rejected");
        assert_eq!(
            FsErrorKind::ProviderContractViolation,
            error.error().kind()
        );
    }

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        invalid_temp_identity: true,
        temp_cleanup_failure: true,
        ..AsyncRecordingConfig::default()
    });
    let Err(file) = ready(
        file_system.create_temp_file(qubit_fs::TempFileOptions::default()),
    ) else {
        panic!("invalid temporary file identity must be rejected");
    };
    let Err(directory) = ready(
        file_system
            .create_temp_directory(qubit_fs::TempDirectoryOptions::default()),
    ) else {
        panic!("invalid temporary directory identity must be rejected");
    };
    for error in [file, directory] {
        assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    }

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        invalid_temp_path: true,
        ..AsyncRecordingConfig::default()
    });
    let Err(invalid_path) = ready(
        file_system.create_temp_file(qubit_fs::TempFileOptions::default()),
    ) else {
        panic!("relative temporary path must be rejected");
    };
    assert_eq!(FsErrorKind::ProviderContractViolation, invalid_path.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        completed_copy: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(
            source.clone(),
            target.clone(),
            CopyOptions {
                durability: qubit_fs::DurabilityRequirement::Required,
                ..CopyOptions::default()
            },
        )
        .expect("durable-copy capability should satisfy preflight");
    let durability = ready(operation.execute())
        .expect_err("non-durable provider outcome must be rejected");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        durability.error().kind()
    );

    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let Err(same) = file_system.begin_copy(
        source.clone(),
        source.clone(),
        CopyOptions::default(),
    ) else {
        panic!("copy source and target must differ");
    };
    assert_eq!(FsErrorKind::InvalidOptions, same.error().kind());

    for config in [
        AsyncRecordingConfig {
            failing_stage: Some(AsyncCopyStage::Stat),
            ..AsyncRecordingConfig::default()
        },
        AsyncRecordingConfig {
            failing_stage: Some(AsyncCopyStage::OpenReader),
            ..AsyncRecordingConfig::default()
        },
        AsyncRecordingConfig {
            failing_stage: Some(AsyncCopyStage::OpenWriter),
            ..AsyncRecordingConfig::default()
        },
        AsyncRecordingConfig {
            stat_kind: Some(FileKind::Directory),
            ..AsyncRecordingConfig::default()
        },
    ] {
        let (file_system, _) = async_recording_file_system(config);
        let mut operation = file_system
            .begin_copy(source.clone(), target.clone(), CopyOptions::default())
            .expect("copy preflight should succeed");
        let failure = ready(operation.execute())
            .expect_err("declined fallback boundary failure must propagate");
        assert_eq!(CopyFailureState::Unchanged, failure.state());
    }

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        writer_open_error: Some(FsErrorKind::AlreadyExists),
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(
            source,
            target,
            CopyOptions {
                conflict: CopyConflictPolicy::Skip,
                ..CopyOptions::default()
            },
        )
        .expect("skip fallback preflight should succeed");
    let outcome =
        ready(operation.execute()).expect("existing target should be skipped");
    assert_eq!(1, outcome.stats().skipped);
}

/// Covers capability gates and path-semantics checks before provider I/O.
#[test]
fn test_async_facade_rejects_unsupported_capabilities_and_path_semantics() {
    let source = path("/source");
    let target = path("/target");

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::Read),
        ..AsyncRecordingConfig::default()
    });
    let error = ready(file_system.open_reader(&source, ReadOptions::default()))
        .expect_err("read capability must be required");
    assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::Write),
        ..AsyncRecordingConfig::default()
    });
    let error =
        ready(file_system.open_writer(&target, WriteOptions::default()))
            .expect_err("write capability must be required");
    assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::CreateDirectory),
        ..AsyncRecordingConfig::default()
    });
    let error = ready(
        file_system
            .create_directory(&target, CreateDirectoryOptions::default()),
    )
    .expect_err("directory-creation capability must be required");
    assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::TempFile),
        ..AsyncRecordingConfig::default()
    });
    let Err(error) = ready(
        file_system.create_temp_file(qubit_fs::TempFileOptions::default()),
    ) else {
        panic!("temporary-file capability must be required");
    };
    assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::TempDirectory),
        ..AsyncRecordingConfig::default()
    });
    let Err(error) = ready(
        file_system
            .create_temp_directory(qubit_fs::TempDirectoryOptions::default()),
    ) else {
        panic!("temporary-directory capability must be required");
    };
    assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());

    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let error =
        ready(file_system.rename(&source, &target, RenameOptions::default()))
            .expect_err("rename capability must be required");
    assert_eq!(FsErrorKind::UnsupportedCapability, error.error().kind());

    let object_key = Path::parse_with_semantics(
        "/provider-literal",
        PathSemantics::ObjectKey,
    )
    .expect("object key should parse");
    let error = ready(file_system.stat(&object_key))
        .expect_err("foreign path semantics must be rejected");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
}

/// Covers successful stream fallback completion after all provider boundaries.
#[test]
fn test_async_facade_stream_fallback_returns_completed_outcome() {
    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let mut operation = file_system
        .begin_copy(path("/source"), path("/target"), CopyOptions::default())
        .expect("copy preflight should succeed");
    let outcome = ready(operation.execute())
        .expect("stream fallback should return its completed outcome");
    assert_eq!(1, outcome.stats().files);
}

/// Covers facade preflight error exits that must remain free of provider I/O.
#[test]
fn test_async_facade_preflight_rejects_invalid_paths_options_and_capabilities()
{
    let source = path("/source");
    let target = path("/target");
    let relative = path("relative");
    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());

    ready(file_system.list(&source, ListOptions::default()))
        .expect("listing should dispatch with the advertised capability");
    assert!(
        ready(file_system.list(&relative, ListOptions::default())).is_err()
    );
    let (without_list, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::List),
        ..AsyncRecordingConfig::default()
    });
    assert!(ready(without_list.list(&source, ListOptions::default())).is_err());
    assert!(
        ready(file_system.open_reader(&relative, ReadOptions::default()))
            .is_err()
    );
    assert!(
        ready(file_system.open_reader(
            &source,
            ReadOptions {
                length: Some(1),
                ..ReadOptions::default()
            },
        ),)
        .is_err()
    );
    let (read_limited, _) = async_recording_file_system(AsyncRecordingConfig {
        range_read: true,
        maximum_read_range_bytes: Some(1),
        ..AsyncRecordingConfig::default()
    });
    assert!(
        ready(read_limited.open_reader(
            &source,
            ReadOptions {
                length: Some(2),
                ..ReadOptions::default()
            },
        ),)
        .is_err()
    );
    assert!(
        ready(file_system.open_writer(&relative, WriteOptions::default()))
            .is_err()
    );
    assert!(
        ready(file_system.open_writer(
            &target,
            WriteOptions {
                atomicity: AtomicityRequirement::Required,
                ..WriteOptions::default()
            },
        ),)
        .is_err()
    );
    assert!(
        ready(
            file_system
                .create_directory(&relative, CreateDirectoryOptions::default()),
        )
        .is_err()
    );

    assert!(
        file_system
            .begin_copy(
                relative.clone(),
                target.clone(),
                CopyOptions::default()
            )
            .is_err()
    );
    assert!(
        file_system
            .begin_copy(
                source.clone(),
                relative.clone(),
                CopyOptions::default()
            )
            .is_err()
    );
    assert!(
        file_system
            .begin_copy(
                source.clone(),
                target.clone(),
                CopyOptions {
                    durability: DurabilityRequirement::Required,
                    ..CopyOptions::default()
                },
            )
            .is_err()
    );
    let (without_copy, _) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::Copy),
        omit_read_and_write: true,
        ..AsyncRecordingConfig::default()
    });
    assert!(
        without_copy
            .begin_copy(source.clone(), target.clone(), CopyOptions::default())
            .is_err()
    );

    assert!(
        ready(file_system.rename(&relative, &target, RenameOptions::default()))
            .is_err()
    );
    assert!(
        ready(file_system.rename(&source, &relative, RenameOptions::default()))
            .is_err()
    );
    assert!(
        ready(file_system.rename(
            &source,
            &target,
            RenameOptions {
                atomicity: AtomicityRequirement::Required,
                ..RenameOptions::default()
            },
        ),)
        .is_err()
    );

    assert!(
        ready(file_system.delete_file(&relative, DeleteOptions::default()))
            .is_err()
    );
    assert!(
        ready(file_system.delete_file(
            &target,
            DeleteOptions {
                recursive: true,
                ..DeleteOptions::default()
            },
        ),)
        .is_err()
    );
    let (without_delete, _) =
        async_recording_file_system(AsyncRecordingConfig {
            omitted_capability: Some(FileSystemCapability::Delete),
            ..AsyncRecordingConfig::default()
        });
    assert!(
        ready(without_delete.delete_file(&target, DeleteOptions::default()),)
            .is_err()
    );

    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let mut temporary = ready(
        file_system.create_temp_file(qubit_fs::TempFileOptions::default()),
    )
    .expect("temporary file should be created");
    assert!(
        ready(temporary.persist(&relative, PersistOptions::default())).is_err()
    );
}

/// Covers a completed native-copy outcome that satisfies the requested policy.
#[test]
fn test_async_facade_returns_valid_completed_copy_outcome() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        completed_copy: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(path("/source"), path("/target"), CopyOptions::default())
        .expect("copy preflight should succeed");
    let outcome = ready(operation.execute())
        .expect("valid completed copy should succeed");
    assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity());
}
