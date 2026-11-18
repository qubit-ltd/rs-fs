// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::thread;
use std::time::Duration;
use std::time::Instant;

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
use qubit_fs_documentation_examples::managed_read_support::shutdown_manager_async;
use qubit_operation::ManagerOptions;
use qubit_operation::OperationManager;
use qubit_operation::OperationOutcome;
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

struct ThreadWake(thread::Thread);

impl std::task::Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match Future::poll(Pin::as_mut(&mut future), &mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

#[test]
fn test_async_shutdown_yields_while_a_managed_operation_is_active() {
    if std::env::var_os("QUBIT_FS_SHUTDOWN_CHILD").is_some() {
        let manager = OperationManager::new(ManagerOptions::default()).unwrap();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let task_gate = Arc::clone(&gate);
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let mut operation = manager
            .try_submit(Default::default(), move |_| {
                started_tx.send(()).unwrap();
                let (lock, changed) = &*task_gate;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = changed.wait(released).unwrap();
                }
                Ok::<_, ()>(())
            })
            .unwrap();
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let mut shutdown = Box::pin(shutdown_manager_async(&manager));
        let mut released = false;
        block_on(std::future::poll_fn(|context| match shutdown.as_mut().poll(context) {
            Poll::Pending if !released => {
                released = true;
                let (lock, changed) = &*gate;
                *lock.lock().unwrap() = true;
                changed.notify_one();
                Poll::Pending
            }
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => Poll::Ready(result),
        }))
        .unwrap();
        assert!(released, "shutdown must yield before the active operation is released");
        assert!(matches!(
            operation.wait_outcome().unwrap(),
            OperationOutcome::Succeeded(())
        ));
        operation.wait_settlement().unwrap();
        return;
    }

    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("test_async_shutdown_yields_while_a_managed_operation_is_active")
        .env("QUBIT_FS_SHUTDOWN_CHILD", "1")
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "async shutdown child failed with {status}");
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("async shutdown blocked the executor thread past the 8-second limit");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
