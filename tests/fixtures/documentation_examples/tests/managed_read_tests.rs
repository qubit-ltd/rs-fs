// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::thread;

use qubit_fs::FsExecutionBinding;
use qubit_fs::FsOwnedReadRequest;
use qubit_fs::Path;
use qubit_fs::ReadAllOptions;
use qubit_fs::ReadConsistencyPolicy;
use qubit_fs_documentation_examples::managed_read::read_report;
use qubit_fs_documentation_examples::managed_read::rejected_request;
use qubit_fs_documentation_examples::managed_read_async::read_report_async;
use qubit_fs_documentation_examples::managed_read_async::rejected_request_async;
use qubit_fs_documentation_examples::managed_read_support::MemoryReadProvider;
use qubit_operation::ManagerOptions;
use qubit_operation::OperationManager;
use qubit_operation::ShutdownMode;

fn run_ready<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        match Future::poll(Pin::as_mut(&mut future), &mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::yield_now(),
        }
    }
}

#[test]
fn test_managed_sync_and_async_examples_read_report_and_settle() {
    assert_eq!(read_report().unwrap(), b"report ready");
    assert_eq!(run_ready(read_report_async()).unwrap(), b"report ready");
}

#[test]
fn test_preparation_rejection_returns_identical_request_without_io() {
    let (request, io_started) = rejected_request();
    assert_eq!(request.path, Path::parse("/report.txt").unwrap());
    assert_eq!(request.options.max_bytes, ReadAllOptions::default().max_bytes);
    assert!(!io_started);

    let (request, io_started) = run_ready(rejected_request_async());
    assert_eq!(request.path, Path::parse("/report.txt").unwrap());
    assert_eq!(request.options.max_bytes, ReadAllOptions::default().max_bytes);
    assert!(!io_started);
}

#[test]
fn test_manager_admission_rejection_returns_request_without_provider_io() {
    let manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let _shutdown = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
    let provider = MemoryReadProvider::new();
    let request = FsOwnedReadRequest {
        path: Path::parse("/same-request").unwrap(),
        options: ReadAllOptions::new(7, ReadConsistencyPolicy::ObserveChanges),
    };
    let failure = match FsExecutionBinding::new(manager.clone()).submit_read_all(&provider, request) {
        Ok(_) => panic!("closed manager must reject submission"),
        Err(failure) => failure,
    };
    let (admission, preparation, request) = failure.into_parts();
    assert!(admission.is_some());
    assert!(preparation.is_none());
    assert_eq!(request.path, Path::parse("/same-request").unwrap());
    assert_eq!(request.options.max_bytes, 7);
    assert_eq!(request.options.consistency, ReadConsistencyPolicy::ObserveChanges);
    assert!(!provider.io_started());
}
