#[cfg(feature = "async")]
use std::future::Future;
#[cfg(feature = "async")]
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
#[cfg(feature = "async")]
use std::task::Context;
#[cfg(feature = "async")]
use std::task::Poll;
#[cfg(feature = "async")]
use std::task::Wake;
#[cfg(feature = "async")]
use std::task::Waker;

use qubit_fs::FsEffectObserver;
use qubit_fs::FsExecutionBinding;
use qubit_fs::FsOperation;
use qubit_fs::FsOperationFailure;
use qubit_fs::FsOperationFailureKind;
use qubit_fs::FsOwnedReadRequest;
use qubit_fs::FsPrepareFailure;
use qubit_fs::Path;
use qubit_fs::ReadAllOptions;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::TreeValidationOptions;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncManagedReadPort;
use qubit_fs::spi::ManagedFsTreeSpi;
use qubit_fs::spi::ManagedReadPort;
use qubit_fs::spi::RandomAccessReadSession;

struct BytesSession {
    opened_bytes: Arc<Vec<u8>>,
    reads: Arc<AtomicUsize>,
}

impl RandomAccessReadSession for BytesSession {
    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> qubit_fs::FsResult<usize> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let start = usize::try_from(offset).unwrap();
        let Some(bytes) = self.opened_bytes.get(start..) else {
            return Ok(0);
        };
        let count = bytes.len().min(output.len());
        output[..count].copy_from_slice(&bytes[..count]);
        Ok(count)
    }
}

#[test]
fn managed_reader_keeps_one_opened_object_and_counts_repeated_ranges_once() {
    let bytes = Arc::new(b"object-A".to_vec());
    let reads = Arc::new(AtomicUsize::new(0));
    let info = OpenedFileInfo::new(
        FileSystemId::new("neutral-fake").unwrap(),
        Path::parse("/item").unwrap(),
    );
    let port = ManagedReadPort::new(
        info,
        8,
        3,
        8,
        Box::new(BytesSession {
            opened_bytes: Arc::clone(&bytes),
            reads: Arc::clone(&reads),
        }),
    );

    // The port performs no path lookup: every segment reaches its opened session.
    let mut first = [0; 3];
    let mut second = [0; 3];
    assert_eq!(3, port.read_at(0, &mut first).unwrap());
    assert_eq!(3, port.read_at(3, &mut second).unwrap());
    assert_eq!(b"obj", &first);
    assert_eq!(b"ect", &second);
    assert_eq!(2, reads.load(Ordering::SeqCst));
    assert_eq!(2, port.read_at(0, &mut first).unwrap());
    assert!(
        port.read_at(0, &mut first).is_err(),
        "repeated ranges count against the shared byte budget"
    );
}

#[cfg(feature = "async")]
struct AsyncBytesSession(Vec<u8>);

#[cfg(feature = "async")]
impl qubit_fs::spi::AsyncRandomAccessReadSession for AsyncBytesSession {
    fn read_at_async<'a>(
        self: Pin<&'a mut Self>,
        offset: u64,
        output: &'a mut [u8],
    ) -> qubit_fs::spi::SpiFuture<'a, qubit_fs::FsResult<usize>> {
        let start = usize::try_from(offset).unwrap();
        let Some(bytes) = self.get_mut().0.get(start..) else {
            return Box::pin(async { Ok(0) });
        };
        let count = bytes.len().min(output.len());
        output[..count].copy_from_slice(&bytes[..count]);
        Box::pin(async move { Ok(count) })
    }
}

#[cfg(feature = "async")]
fn run_ready<F: Future>(future: F) -> F::Output {
    struct NoopWake;
    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match Future::poll(Pin::as_mut(&mut future), &mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[cfg(feature = "async")]
#[test]
fn managed_async_reader_runs_without_an_executor_dependency() {
    let info = OpenedFileInfo::new(
        FileSystemId::new("neutral-fake").unwrap(),
        Path::parse("/async-item").unwrap(),
    );
    let mut port = Box::pin(AsyncManagedReadPort::new(
        info,
        4,
        2,
        4,
        Box::pin(AsyncBytesSession(b"data".to_vec())),
    ));
    // The async feature adds Future APIs; this controlled fake completes inline
    // and does not require or start a runtime thread.
    let mut output = [0; 2];
    assert_eq!(2, run_ready(port.as_mut().read_at_async(0, &mut output)).unwrap());
    assert_eq!(b"da", &output);
}

struct FakeTree;
impl ManagedFsTreeSpi for FakeTree {
    fn prepare_tree_publish(
        &self,
        request: qubit_fs::FsOwnedTreePublishRequest,
    ) -> Result<
        Arc<dyn qubit_fs::spi::PreparedFsTreePublish>,
        qubit_fs::FsPrepareFailure<qubit_fs::FsOwnedTreePublishRequest>,
    > {
        Err(qubit_fs::FsPrepareFailure {
            kind: qubit_fs::execution::FsPrepareFailureKind::Unsupported,
            request,
        })
    }
}

#[test]
fn staged_resource_cleanup_runs_once_and_does_not_double_count() {
    use qubit_operation::ManagerOptions;
    use qubit_operation::OperationOptions;
    use qubit_operation::OperationOutcome;
    use qubit_operation::ResourceScopeOptions;
    use qubit_operation::ShutdownMode;

    let manager = qubit_operation::OperationManager::new(ManagerOptions::default()).unwrap();
    let cleanup_calls = Arc::new(AtomicUsize::new(0));
    let cleanup_counter = Arc::clone(&cleanup_calls);
    let mut operation = manager
        .try_submit(OperationOptions::default(), move |operation_scope| {
            let scope = operation_scope
                .resource_scope(ResourceScopeOptions { capacity: 1 })
                .unwrap();
            let reservation = FakeTree.reserve_staged_directory(&scope).unwrap();
            let staged = FakeTree
                .commit_staged_directory(
                    reservation,
                    Path::parse("/stage").unwrap(),
                    TreeValidationOptions::default(),
                    Box::new(move || {
                        cleanup_counter.fetch_add(1, Ordering::SeqCst);
                        Box::pin(async { Ok(()) })
                    }),
                )
                .unwrap();
            drop(staged);
            scope.end();
            Ok::<_, ()>(())
        })
        .unwrap();

    assert!(matches!(
        operation.wait_outcome().unwrap(),
        OperationOutcome::Succeeded(())
    ));
    let settlement = operation.wait_settlement().unwrap();
    assert!(settlement.all_released());
    assert_eq!(1, cleanup_calls.load(Ordering::SeqCst), "cleanup must run exactly once");
    assert_eq!(
        1,
        settlement.resource_reports.len(),
        "one staged owner is counted exactly once"
    );
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}

#[test]
fn public_managed_api_and_typed_provider_error_keep_the_original_source() {
    fn accepts_root_api(_: Option<FsEffectObserver>, _: Option<FsExecutionBinding>, _: Option<FsOperation<()>>) {}
    accepts_root_api(None, None, None);
    let request = FsOwnedReadRequest {
        path: Path::parse("/input").unwrap(),
        options: ReadAllOptions::default(),
    };
    let failure = FsPrepareFailure {
        kind: qubit_fs::FsPrepareFailureKind::Unsupported,
        request,
    };
    assert_eq!("/input", failure.request.path.as_str());

    let source = Arc::new(qubit_fs::FsError::new(
        qubit_fs::error::FsErrorKind::Other,
        qubit_fs::error::FsOperation::Read,
        "fake provider failure",
    ));
    let failure = FsOperationFailure::new(
        FsOperationFailureKind::Provider(Arc::clone(&source)),
        qubit_fs::FsEffectState::Unchanged,
    );
    assert!(
        std::error::Error::source(&failure)
            .and_then(|error| error.downcast_ref::<qubit_fs::FsError>())
            .is_some_and(|error| error.to_string() == source.to_string())
    );
}
