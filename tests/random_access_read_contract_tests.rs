//! Public random-access session contracts, including the path-replacement model.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[path = "common/random_access.rs"]
mod random_access_common;
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::{
    FileKind, FileMetadata, FileSystemCapabilities, FileSystemCapability, FileSystemId,
    FileSystemInfo, FileSystemLimit, FileSystemLimits, SymlinkPolicy,
};
use qubit_fs::path::{PathConstraints, PathSemantics};
use qubit_fs::read::ReadOptions;
use qubit_fs::spi::{
    FileSystemSpi, OpenReaderRequest, OpenedRandomAccessReader, ProviderOperation,
    ProviderOperations, ProviderProperties, RandomAccessReadSession, StatRequest, StatResponse,
};
use qubit_fs::{FileSystem, FsResult, Path};
const ID: &str = "random-access-contract";
struct Provider {
    opens: Arc<AtomicUsize>,
    limit: FileSystemLimit,
}
struct Session(Vec<u8>);
impl RandomAccessReadSession for Session {
    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> FsResult<usize> {
        let start = offset as usize;
        let n = out.len().min(self.0.len().saturating_sub(start));
        out[..n].copy_from_slice(&self.0[start..start + n]);
        Ok(n)
    }
}
impl FileSystemSpi for Provider {
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
            FileSystemLimits::unknown().with_max_read_range_bytes(self.limit),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }
    fn stat(&self, r: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            r.path().clone(),
            FileMetadata::new(FileKind::File),
        ))
    }
    fn open_random_access_reader(
        &self,
        r: OpenReaderRequest<'_>,
    ) -> FsResult<OpenedRandomAccessReader> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        let info = random_access_common::opened_info(ID, r.path(), 6);
        Ok(OpenedRandomAccessReader::new(
            info,
            Box::new(Session(b"object".to_vec())),
        ))
    }
}
#[test]
fn test_random_access_reuses_opened_object_across_nonsequential_reads() {
    let opens = Arc::new(AtomicUsize::new(0));
    let fs = FileSystem::from_spi(Provider {
        opens: opens.clone(),
        limit: FileSystemLimit::Unknown,
    })
    .unwrap();
    let mut reader = fs
        .open_random_access_reader(&Path::parse("/same").unwrap(), ReadOptions::default())
        .unwrap();
    assert_eq!(6, reader.len());
    let mut tail = [0; 2];
    assert_eq!(2, reader.read_at(4, &mut tail).unwrap());
    let mut head = [0; 2];
    assert_eq!(2, reader.read_at(0, &mut head).unwrap());
    assert_eq!(*b"ct", tail);
    assert_eq!(*b"ob", head);
    assert_eq!(1, opens.load(Ordering::SeqCst));
    // Model evidence: replacing a path between legacy opens could return A then B;
    // this retained session instead continues reading the opened A object.
}
#[test]
fn test_random_access_rejects_noncomplete_options_before_provider_open() {
    let opens = Arc::new(AtomicUsize::new(0));
    let fs = FileSystem::from_spi(Provider {
        opens: opens.clone(),
        limit: FileSystemLimit::Unknown,
    })
    .unwrap();
    let error = fs
        .open_random_access_reader(
            &Path::parse("/same").unwrap(),
            ReadOptions::default().with_offset(Some(2)),
        )
        .unwrap_err();
    assert_eq!(FsErrorKind::InvalidOptions, error.kind());
    assert_eq!(0, opens.load(Ordering::SeqCst));
}

#[test]
fn test_random_access_eof_empty_overflow_and_limit_policy() {
    let opens = Arc::new(AtomicUsize::new(0));
    let fs = FileSystem::from_spi(Provider {
        opens: opens.clone(),
        limit: FileSystemLimit::Maximum(2),
    })
    .unwrap();
    let mut reader = fs
        .open_random_access_reader(&Path::parse("/same").unwrap(), ReadOptions::default())
        .unwrap();
    assert_eq!(0, reader.read_at(0, &mut []).unwrap());
    assert_eq!(0, reader.read_at(6, &mut [0; 1]).unwrap());
    assert_eq!(
        FsErrorKind::ResourceLimitExceeded,
        reader.read_at(0, &mut [0; 3]).unwrap_err().kind()
    );
    assert_eq!(
        FsErrorKind::InvalidOptions,
        reader.read_at(u64::MAX, &mut [0; 2]).unwrap_err().kind()
    );
}
