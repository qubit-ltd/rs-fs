//! Runtime-neutral async random-access session contracts.
use std::future::Future;
use std::future::poll_fn;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::task::{Context, Poll, Waker};
#[path = "common/random_access.rs"]
mod random_access_common;
use qubit_fs::metadata::{
    FileKind, FileMetadata, FileSystemCapabilities, FileSystemCapability, FileSystemId,
    FileSystemInfo, FileSystemLimits, SymlinkPolicy,
};
use qubit_fs::path::{PathConstraints, PathSemantics};
use qubit_fs::read::ReadOptions;
use qubit_fs::spi::{
    AsyncFileSystemSpi, AsyncRandomAccessReadSession, OpenReaderRequest,
    OpenedAsyncRandomAccessReader, ProviderOperation, ProviderOperations, ProviderProperties,
    SpiFuture, StatRequest, StatResponse,
};
use qubit_fs::{AsyncFileSystem, FsResult, Path};
const ID: &str = "async-random-access-contract";
struct Provider(Arc<AtomicUsize>);
struct Session(Arc<AtomicUsize>, Vec<u8>);
impl AsyncRandomAccessReadSession for Session {
    fn read_at_async<'a>(
        self: Pin<&'a mut Self>,
        offset: u64,
        out: &'a mut [u8],
    ) -> SpiFuture<'a, FsResult<usize>> {
        Box::pin(async move {
            let this = self.get_mut();
            let call = this.0.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                poll_fn(|_| Poll::<()>::Pending).await;
            }
            let start = offset as usize;
            let n = out.len().min(this.1.len().saturating_sub(start));
            out[..n].copy_from_slice(&this.1[start..start + n]);
            Ok(n)
        })
    }
}
impl AsyncFileSystemSpi for Provider {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new(ID).unwrap(),
                "fixture",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::OpenReader)
                .with(ProviderOperation::OpenRandomAccessReader),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }
    fn stat<'a>(&'a self, r: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async move {
            Ok(StatResponse::new(
                r.path().clone(),
                FileMetadata::new(FileKind::File),
            ))
        })
    }
    fn open_random_access_reader<'a>(
        &'a self,
        r: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncRandomAccessReader>> {
        let n = self.0.clone();
        Box::pin(async move {
            let info = random_access_common::opened_info(ID, r.path(), 6);
            Ok(OpenedAsyncRandomAccessReader::new(
                info,
                Box::new(Session(n, b"object".to_vec())),
            ))
        })
    }
}
fn ready<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    match f.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("expected ready"),
    }
}
#[test]
fn test_async_random_access_reuses_session_after_pending_read_cancellation() {
    let calls = Arc::new(AtomicUsize::new(0));
    let fs = AsyncFileSystem::from_spi(Provider(calls.clone())).unwrap();
    let mut reader =
        ready(fs.open_random_access_reader(&Path::parse("/same").unwrap(), ReadOptions::default()))
            .unwrap();
    let mut out = [0; 2];
    {
        let mut pending = std::pin::pin!(reader.read_at(4, &mut out));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(pending.as_mut().poll(&mut cx).is_pending());
    }
    assert_eq!([0; 2], out);
    assert_eq!(1, calls.load(Ordering::SeqCst));
    assert_eq!(2, ready(reader.read_at(0, &mut out)).unwrap());
    assert_eq!(*b"ob", out);
    assert_eq!(2, calls.load(Ordering::SeqCst));
}
