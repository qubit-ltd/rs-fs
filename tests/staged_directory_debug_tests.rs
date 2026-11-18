// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::process::Command;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use qubit_fs::Path;
use qubit_fs::metadata::ResourceVersion;
use qubit_fs::metadata::TreeValidationOptions;
use qubit_fs::spi::ManagedFsTreeSpi;
use qubit_operation::ManagerOptions;
use qubit_operation::OperationManager;
use qubit_operation::OperationOptions;
use qubit_operation::OperationOutcome;
use qubit_operation::ResourceScopeOptions;
use qubit_operation::ShutdownMode;

struct Provider;

impl ManagedFsTreeSpi for Provider {
    fn prepare_tree_publish(
        &self,
        _: qubit_fs::FsOwnedTreePublishRequest,
    ) -> Result<
        std::sync::Arc<dyn qubit_fs::spi::PreparedFsTreePublish>,
        qubit_fs::FsPrepareFailure<qubit_fs::FsOwnedTreePublishRequest>,
    > {
        unreachable!("debug formatting test does not publish")
    }
}

#[test]
fn test_staged_directory_debug_does_not_deadlock() {
    if std::env::var_os("QUBIT_FS_DEBUG_CHILD").is_some() {
        let manager = OperationManager::new(ManagerOptions::default()).unwrap();
        let mut operation = manager
            .try_submit(OperationOptions::default(), |operation_scope| {
                let scope = operation_scope
                    .resource_scope(ResourceScopeOptions { capacity: 1 })
                    .unwrap();
                let reservation = Provider.reserve_staged_directory(&scope).unwrap();
                let staged = Provider
                    .commit_staged_directory(
                        reservation,
                        Path::parse("/stage").unwrap(),
                        TreeValidationOptions::default(),
                        Box::new(|| Box::pin(async { Ok(()) })),
                    )
                    .unwrap();
                staged.validate_entry("entry", "entry", false, 7).unwrap();
                staged.record_validated_version(ResourceVersion::from("version-1"));
                let debug = format!("{:?}", *staged);
                drop(staged);
                scope.end();
                Ok::<_, ()>(debug)
            })
            .unwrap();
        let debug = match operation.wait_outcome().unwrap() {
            OperationOutcome::Succeeded(debug) => debug,
            other => panic!("unexpected operation outcome: {other:?}"),
        };
        assert!(debug.contains("root: Path") && debug.contains("/stage"), "{debug}");
        assert!(debug.contains("entries: 1"), "{debug}");
        assert!(debug.contains("content_bytes: 7"), "{debug}");
        assert!(debug.contains("validated: true"), "{debug}");
        assert!(!debug.contains("cleanup"), "{debug}");
        operation.wait_settlement().unwrap();
        let _shutdown_report = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
        return;
    }

    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("test_staged_directory_debug_does_not_deadlock")
        .env("QUBIT_FS_DEBUG_CHILD", "1")
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "debug child failed with {status}");
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("formatting a staged directory deadlocked past the 5-second limit");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
