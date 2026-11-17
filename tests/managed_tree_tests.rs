// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use qubit_fs::FsOwnedTreePublishRequest;
use qubit_fs::FsPrepareFailure;
use qubit_fs::Path;
use qubit_fs::directory::TreePublishOutcome;
use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation as FsErrorOperation;
use qubit_fs::execution::FsPrepareFailureKind;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::AtomicityRequirement;
use qubit_fs::metadata::DurabilityRequirement;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemCapabilityProfile;
use qubit_fs::metadata::FileSystemCapabilitySupport;
use qubit_fs::metadata::PublicationMethod;
use qubit_fs::metadata::StorageObservation;
use qubit_fs::metadata::TreePublishOptions;
use qubit_fs::metadata::TreeValidationOptions;
use qubit_fs::spi::ManagedFsTreeSpi;
use qubit_fs::spi::PreparedFsTreePublish;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_operation::ManagerOptions;
use qubit_operation::OperationManager;
use qubit_operation::OperationOptions;
use qubit_operation::OperationOutcome;
use qubit_operation::RecoveryPhase;
use qubit_operation::ResourceScopeOptions;
use qubit_operation::ShutdownMode;

struct FakeTreeProvider;

struct NonCasTreeProvider {
    preparations: Arc<AtomicUsize>,
    effects: Arc<AtomicUsize>,
}

impl ManagedFsTreeSpi for NonCasTreeProvider {
    fn prepare_tree_publish(
        &self,
        request: FsOwnedTreePublishRequest,
    ) -> Result<Arc<dyn PreparedFsTreePublish>, FsPrepareFailure<FsOwnedTreePublishRequest>> {
        self.preparations.fetch_add(1, Ordering::SeqCst);
        if request.publication.atomicity == AtomicityRequirement::Required
            && request.validated_version.is_some()
        {
            return Err(FsPrepareFailure {
                kind: FsPrepareFailureKind::Unsupported,
                request,
            });
        }
        self.effects.fetch_add(1, Ordering::SeqCst);
        unreachable!("this fake provider is used only for unsupported Required requests")
    }
}

impl ManagedFsTreeSpi for FakeTreeProvider {
    fn prepare_tree_publish(
        &self,
        _request: FsOwnedTreePublishRequest,
    ) -> Result<Arc<dyn PreparedFsTreePublish>, FsPrepareFailure<FsOwnedTreePublishRequest>> {
        unreachable!("focused owner tests do not activate publication")
    }
}

macro_rules! run_staged {
    ($staged:ident, $root:expr, $options:expr, $cleanup:expr, $body:block) => {{
        let manager = OperationManager::new(ManagerOptions::default()).unwrap();
        let mut operation = manager
            .try_submit(OperationOptions::default(), move |operation_scope| {
                let scope = operation_scope
                    .resource_scope(ResourceScopeOptions { capacity: 4 })
                    .expect("resource scope should be available");
                let reservation = FakeTreeProvider
                    .reserve_staged_directory(&scope)
                    .expect("resource cleanup capacity should be reserved");
                let $staged = FakeTreeProvider
                    .commit_staged_directory(
                        reservation,
                        Path::parse($root).unwrap(),
                        $options,
                        Box::new($cleanup),
                    )
                    .expect("staged tree should register in the resource scope");
                $body
                drop($staged);
                scope.end();
                Ok::<(), ()>(())
            })
            .expect("operation should be accepted");
        let outcome = operation.wait_outcome().expect("operation outcome available");
        assert!(matches!(outcome, OperationOutcome::Succeeded(())));
        let settlement = operation.wait_settlement().expect("settlement available");
        let _shutdown_report = manager
            .request_shutdown(ShutdownMode::Drain)
            .wait()
            .expect("manager should shut down");
        settlement
    }};
}

#[test]
fn test_operation_presence_is_only_conditional_and_sync_async_are_independent() {
    let sync = ProviderOperations::new()
        .with(ProviderOperation::PublishTree)
        .with(ProviderOperation::ObserveStorage);
    let asynchronous = ProviderOperations::new().with(ProviderOperation::List);

    let profile = FileSystemCapabilityProfile::from_operations(sync, asynchronous);
    assert_eq!(
        profile
            .synchronous
            .support(FileSystemCapability::StagedTreePublish),
        FileSystemCapabilitySupport::Conditional,
    );
    assert_eq!(
        profile
            .synchronous
            .support(FileSystemCapability::AtomicTreeCopy),
        FileSystemCapabilitySupport::Unsupported,
    );
    assert_eq!(
        profile
            .synchronous
            .support(FileSystemCapability::StorageObservation),
        FileSystemCapabilitySupport::Conditional,
    );
    assert_eq!(
        profile
            .asynchronous
            .support(FileSystemCapability::StagedTreePublish),
        FileSystemCapabilitySupport::Unsupported,
    );
    assert_eq!(
        profile.asynchronous.support(FileSystemCapability::List),
        FileSystemCapabilitySupport::Conditional,
    );
}

#[test]
fn test_unknown_storage_capacity_is_a_valid_observation() {
    let observed = StorageObservation::Unknown;
    assert_eq!(observed, StorageObservation::Unknown);
}

#[test]
fn test_caller_guarantees_are_preserved_by_profile_construction() {
    let declared = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::List)
        .with_conditional(FileSystemCapability::StagedTreePublish);
    let profile = FileSystemCapabilityProfile::new(declared, FileSystemCapabilities::new());
    assert!(profile.synchronous.guarantees(FileSystemCapability::List));
    assert_eq!(
        profile
            .synchronous
            .support(FileSystemCapability::StagedTreePublish),
        FileSystemCapabilitySupport::Conditional,
    );
}

#[test]
fn test_validation_batches_enforce_record_and_byte_limits() {
    let options = TreeValidationOptions::default();
    assert!(options.validate_batch(&vec![1; 128]).is_ok());
    assert!(options.validate_batch(&vec![1; 129]).is_err());
    assert!(options.validate_batch(&[256 * 1024]).is_ok());
    assert!(options.validate_batch(&[256 * 1024 + 1]).is_err());
    assert!(options.validate_batch(&[128 * 1024, 128 * 1024]).is_ok());
    assert!(
        options
            .validate_batch(&[128 * 1024, 128 * 1024 + 1])
            .is_err()
    );
}

#[test]
fn test_entry_validation_rejects_escapes_links_and_normalized_conflicts() {
    let options = TreeValidationOptions {
        max_entries: 2,
        max_depth: 2,
        max_content_bytes: 3,
    };
    let mut normalized = BTreeSet::new();
    let valid = |path: &str, key: &str, bytes, seen: &mut BTreeSet<String>| {
        options.validate_entry(path, key, false, seen.len(), 0, bytes, seen)
    };

    assert!(valid("a/b", "a/b", 3, &mut normalized).is_ok());
    assert!(valid("a/c", "a/c", 0, &mut normalized).is_ok());
    assert!(valid("a/d", "a/d", 0, &mut normalized).is_err());
    assert!(valid("../escape", "escape", 0, &mut normalized).is_err());
    assert!(valid("/absolute", "/absolute", 0, &mut normalized).is_err());
    assert!(valid("a/b/c", "a/b/c", 0, &mut normalized).is_err());
    assert!(valid("safe", "../normalized-escape", 0, &mut normalized).is_err());
    assert!(
        options
            .validate_entry("link", "link", true, 0, 0, 0, &mut normalized)
            .is_err()
    );
    assert!(valid("different", "a/b", 0, &mut normalized).is_err());
    assert!(valid("unknown-length", "unknown-length", 4, &mut normalized).is_err());
    assert!(
        options
            .validate_entry("aggregate", "aggregate", false, 1, 2, 2, &mut normalized)
            .is_err()
    );
}

#[test]
fn test_validation_checks_deadline_and_stop_before_accepting_more_work() {
    let options = TreeValidationOptions::default();
    let now = Instant::now();
    assert!(
        options
            .check_active(now, Some(now + Duration::from_secs(1)), false)
            .is_ok()
    );
    assert!(options.check_active(now, Some(now), false).is_err());
    assert!(options.check_active(now, None, true).is_err());
}

#[test]
fn test_required_tree_guarantees_reject_before_publication_when_unsupported() {
    let required = TreePublishOptions {
        atomicity: AtomicityRequirement::Required,
        durability: DurabilityRequirement::Required,
    };
    let error = required
        .validate_capabilities(&FileSystemCapabilities::new())
        .expect_err("unsupported required guarantees must fail in preflight");
    assert_eq!(
        error.required_capability(),
        Some(FileSystemCapability::AtomicTreeCopy),
    );

    let conditional = FileSystemCapabilities::new()
        .with_conditional(FileSystemCapability::AtomicTreeCopy)
        .with_conditional(FileSystemCapability::DurableTreeCopy);
    assert!(required.validate_capabilities(&conditional).is_ok());
}

#[test]
fn test_tree_outcomes_preserve_partial_publication_facts_and_reject_false_atomicity() {
    let partial = TreePublishOutcome {
        entries_published: 2,
        content_bytes: 17,
        atomicity: AchievedAtomicity::NonAtomic,
        method: PublicationMethod::ServerSideCopy,
        durable: false,
    };
    assert!(partial.facts_are_consistent());
    assert_eq!(partial.entries_published, 2);
    assert!(!partial.durable);
    let required = TreePublishOptions {
        atomicity: AtomicityRequirement::Required,
        durability: DurabilityRequirement::Required,
    };
    assert!(!partial.meets_requirements(&required));

    let impossible = TreePublishOutcome {
        entries_published: 3,
        content_bytes: 29,
        atomicity: AchievedAtomicity::Atomic,
        method: PublicationMethod::ServerSideCopy,
        durable: true,
    };
    assert!(!impossible.facts_are_consistent());
    assert_eq!(impossible.atomicity, AchievedAtomicity::Atomic);
}

#[test]
fn test_staged_owner_retains_cleanup_after_validation_failure() {
    let cleanups = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&cleanups);
    let settlement = run_staged!(
        staged,
        "/stage",
        TreeValidationOptions::default(),
        move || {
            let counted = Arc::clone(&counted);
            Box::pin(async move {
                counted.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        },
        {
            assert!(
                staged
                    .validate_entry("../escape", "escape", false, 0)
                    .is_err()
            );
        }
    );
    assert_eq!(cleanups.load(Ordering::SeqCst), 1);
    assert!(settlement.recoveries.is_empty());
}

#[test]
fn test_failed_cleanup_keeps_owner_for_retry() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&attempts);
    let manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let mut operation = manager
        .try_submit(OperationOptions::default(), move |operation_scope| {
            let scope = operation_scope
                .resource_scope(ResourceScopeOptions { capacity: 4 })
                .unwrap();
            let reservation = FakeTreeProvider.reserve_staged_directory(&scope).unwrap();
            let staged = FakeTreeProvider
                .commit_staged_directory(
                    reservation,
                    Path::parse("/stage").unwrap(),
                    TreeValidationOptions::default(),
                    Box::new(move || {
                        let counted = Arc::clone(&counted);
                        Box::pin(async move {
                            if counted.fetch_add(1, Ordering::SeqCst) == 0 {
                                Err(FsError::new(
                                    FsErrorKind::Other,
                                    FsErrorOperation::Other,
                                    "temporary cleanup failure",
                                ))
                            } else {
                                Ok(())
                            }
                        })
                    }),
                )
                .unwrap();
            drop(staged);
            scope.end();
            Ok::<(), ()>(())
        })
        .unwrap();
    assert!(matches!(
        operation.wait_outcome().unwrap(),
        OperationOutcome::Succeeded(())
    ));
    let settlement = operation.wait_settlement().unwrap();
    assert_eq!(settlement.recoveries.len(), 1);
    let recovery = &settlement.recoveries[0];
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    let report = recovery.try_retry().unwrap().wait().unwrap();
    assert_eq!(report.recovery.phase, RecoveryPhase::Resolved);
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    let _shutdown_report = manager
        .request_shutdown(ShutdownMode::Drain)
        .wait()
        .expect("manager should shut down after recovery");
}

#[test]
fn test_staged_path_prefix_checks_use_components() {
    run_staged!(
        staged,
        "/bucket/tree",
        TreeValidationOptions::default(),
        || Box::pin(async { Ok(()) }),
        {
            assert!(staged.contains_path(&Path::parse("/bucket/tree").unwrap()));
            assert!(staged.contains_path(&Path::parse("/bucket/tree/child").unwrap()));
            assert!(!staged.contains_path(&Path::parse("/bucket/treehouse").unwrap()));
        }
    );
}

#[test]
fn test_changed_tree_version_blocks_required_publish_and_keeps_cleanup_owner() {
    let preparations = Arc::new(AtomicUsize::new(0));
    let effects = Arc::new(AtomicUsize::new(0));
    let no_cas_provider = NonCasTreeProvider {
        preparations: Arc::clone(&preparations),
        effects: Arc::clone(&effects),
    };
    let required = TreePublishOptions {
        atomicity: AtomicityRequirement::Required,
        durability: DurabilityRequirement::NotRequired,
    };
    let capabilities =
        FileSystemCapabilities::new().with_conditional(FileSystemCapability::AtomicTreeCopy);
    run_staged!(
        staged,
        "/stage",
        TreeValidationOptions::default(),
        || Box::pin(async { Ok(()) }),
        {
            staged.record_validated_version("version-before-validation".into());
            assert!(
                staged
                    .check_publish_version(Some(&"changed-after-validation".into()), &required)
                    .is_err()
            );
            assert!(
                staged
                    .publish_request(
                        Path::parse("/destination").unwrap(),
                        required,
                        Some(&"changed-after-validation".into()),
                        &capabilities,
                    )
                    .is_err()
            );
            assert!(
                staged
                    .check_publish_version(Some(&"version-before-validation".into()), &required)
                    .is_ok()
            );
            let request = staged
                .publish_request(
                    Path::parse("/destination").unwrap(),
                    required,
                    Some(&"version-before-validation".into()),
                    &capabilities,
                )
                .expect("unchanged validation version produces a publish request");
            assert_eq!(
                request.validated_version,
                Some("version-before-validation".into())
            );
            let rejection = match no_cas_provider.prepare_tree_publish(request) {
                Err(rejection) => rejection,
                Ok(_) => panic!("provider without atomic version comparison must reject"),
            };
            assert_eq!(
                rejection.request.validated_version,
                Some("version-before-validation".into())
            );
        }
    );
    assert_eq!(preparations.load(Ordering::SeqCst), 1);
    assert_eq!(effects.load(Ordering::SeqCst), 0);
}

#[test]
fn test_durability_requirement_does_not_require_a_validation_version() {
    let durable = TreePublishOptions {
        atomicity: AtomicityRequirement::NotRequired,
        durability: DurabilityRequirement::Required,
    };
    run_staged!(
        staged,
        "/stage",
        TreeValidationOptions::default(),
        || Box::pin(async { Ok(()) }),
        {
            assert!(staged.check_publish_version(None, &durable).is_ok());
        }
    );
}

#[test]
fn test_required_unsupported_publish_request_is_rejected_before_provider_prepare() {
    let required = TreePublishOptions {
        atomicity: AtomicityRequirement::Required,
        durability: DurabilityRequirement::NotRequired,
    };
    run_staged!(
        staged,
        "/stage",
        TreeValidationOptions::default(),
        || Box::pin(async { Ok(()) }),
        {
            assert!(
                staged
                    .publish_request(
                        Path::parse("/destination").unwrap(),
                        required,
                        None,
                        &FileSystemCapabilities::new(),
                    )
                    .is_err()
            );
        }
    );
}

#[test]
fn test_publication_releases_cleanup_only_after_consistent_success() {
    let cleanups = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&cleanups);
    run_staged!(
        staged,
        "/stage",
        TreeValidationOptions::default(),
        move || {
            let counted = Arc::clone(&counted);
            Box::pin(async move {
                counted.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        },
        {
            let required = TreePublishOptions {
                atomicity: AtomicityRequirement::Required,
                durability: DurabilityRequirement::Required,
            };
            let partial = TreePublishOutcome {
                entries_published: 1,
                content_bytes: 1,
                atomicity: AchievedAtomicity::NonAtomic,
                method: PublicationMethod::ServerSideCopy,
                durable: false,
            };
            assert!(staged.publication_succeeded(&partial, &required).is_err());
            assert_eq!(cleanups.load(Ordering::SeqCst), 0);

            let complete = TreePublishOutcome {
                entries_published: 1,
                content_bytes: 1,
                atomicity: AchievedAtomicity::Atomic,
                method: PublicationMethod::AtomicRename,
                durable: true,
            };
            assert!(staged.publication_succeeded(&complete, &required).is_ok());
            assert_eq!(cleanups.load(Ordering::SeqCst), 0);
        }
    );
}
