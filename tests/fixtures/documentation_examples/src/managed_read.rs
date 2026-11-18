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
use qubit_operation::ManagerOptions;
use qubit_operation::OperationManager;
use qubit_operation::OperationOutcome;
use qubit_operation::ShutdownMode;

pub fn read_report() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let manager = OperationManager::new(ManagerOptions::default())?;
    let provider = MemoryReadProvider::new();
    let binding = FsExecutionBinding::new(manager.clone());
    let request = FsOwnedReadRequest {
        path: Path::parse("/report.txt")?,
        options: ReadAllOptions::default(),
    };
    let mut operation = binding
        .submit_read_all(&provider, request)
        .map_err(|failure| std::io::Error::other(format!("managed read rejected: {failure:?}")))?;
    let outcome = operation.wait_outcome()?;
    let bytes = match outcome {
        OperationOutcome::Succeeded(bytes) => bytes,
        other => return Err(format!("unexpected managed read outcome: {other:?}").into()),
    };
    assert_eq!(bytes, b"report ready");
    operation.wait_settlement()?;
    let _shutdown = manager.request_shutdown(ShutdownMode::Drain).wait()?;
    Ok(bytes)
}

pub fn rejected_request() -> (FsOwnedReadRequest, bool) {
    let manager = OperationManager::new(ManagerOptions::default()).expect("manager creation succeeds");
    let provider = MemoryReadProvider::rejecting();
    let binding = FsExecutionBinding::new(manager.clone());
    let request = FsOwnedReadRequest {
        path: Path::parse("/report.txt").expect("valid path"),
        options: ReadAllOptions::default(),
    };
    let failure = match binding.submit_read_all(&provider, request) {
        Ok(_) => panic!("provider preparation must reject"),
        Err(failure) => failure,
    };
    let (admission, preparation, request) = failure.into_parts();
    assert!(admission.is_none());
    assert!(preparation.is_some());
    let _shutdown = manager
        .request_shutdown(ShutdownMode::Drain)
        .wait()
        .expect("manager shuts down");
    (request, provider.io_started())
}
