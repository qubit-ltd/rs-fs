// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FsExecutionBinding;
use qubit_fs::FsOwnedReadRequest;
use qubit_fs::Path;
use qubit_fs::ReadAllOptions;
use qubit_fs_documentation_examples::managed_read_support::MemoryReadProvider;
use qubit_fs_documentation_examples::managed_read_support::shutdown_manager_async;
use qubit_operation::ManagerOptions;
use qubit_operation::OperationManager;
use qubit_operation::OperationOutcome;

pub async fn read_report_async() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let manager = OperationManager::new(ManagerOptions::default())?;
    let provider = MemoryReadProvider::new();
    let binding = FsExecutionBinding::new(manager.clone());
    let request = FsOwnedReadRequest {
        path: Path::parse("/report.txt")?,
        options: ReadAllOptions::default(),
    };
    let mut operation = binding
        .submit_read_all_async(&provider, request)
        .map_err(|failure| std::io::Error::other(format!("managed read rejected: {failure:?}")))?;
    let outcome = operation.wait_outcome().await?;
    let bytes = match outcome {
        OperationOutcome::Succeeded(bytes) => bytes,
        other => return Err(format!("unexpected managed read outcome: {other:?}").into()),
    };
    assert_eq!(bytes, b"report ready");
    let _settlement = operation.wait_settlement().await;
    shutdown_manager_async(&manager).await?;
    Ok(bytes)
}

pub async fn rejected_request_async() -> (FsOwnedReadRequest, bool) {
    let manager = OperationManager::new(ManagerOptions::default()).expect("manager creation succeeds");
    let provider = MemoryReadProvider::rejecting();
    let binding = FsExecutionBinding::new(manager.clone());
    let request = FsOwnedReadRequest {
        path: Path::parse("/report.txt").expect("valid path"),
        options: ReadAllOptions::default(),
    };
    let failure = match binding.submit_read_all_async(&provider, request) {
        Ok(_) => panic!("provider preparation must reject"),
        Err(failure) => failure,
    };
    let (_, _, request) = failure.into_parts();
    shutdown_manager_async(&manager).await.expect("manager shuts down");
    (request, provider.io_started())
}
