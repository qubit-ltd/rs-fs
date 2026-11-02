//! Provider-opened asynchronous random-access reader envelope.
use super::AsyncRandomAccessReadSession;
use crate::metadata::OpenedFileInfo;

/// Opened object metadata and its bound asynchronous session.
///
/// The metadata must describe the same object as the session. The facade
/// validates its identity and requires file kind and length before exposing a
/// reader.
///
/// # Examples
///
/// ```
/// use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::{AsyncRandomAccessReadSession, OpenedAsyncRandomAccessReader, SpiFuture};
/// use qubit_fs::{FsError, FsResult};
/// use std::pin::Pin;
///
/// struct Empty;
/// impl AsyncRandomAccessReadSession for Empty {
///     fn read_at_async<'a>(self: Pin<&'a mut Self>, _: u64, _: &'a mut [u8]) -> SpiFuture<'a, FsResult<usize>> {
///         Box::pin(async { Ok(0) })
///     }
/// }
/// let info = OpenedFileInfo::new(FileSystemId::new("fixture")?, Path::parse("/x")?)
///     .with_metadata(FileMetadata::new(FileKind::File).with_len(Some(0)));
/// let _opened = OpenedAsyncRandomAccessReader::new(info, Box::new(Empty));
/// # Ok::<(), FsError>(())
/// ```
pub struct OpenedAsyncRandomAccessReader {
    info: OpenedFileInfo,
    session: Box<dyn AsyncRandomAccessReadSession>,
}
impl OpenedAsyncRandomAccessReader {
    /// Creates an envelope from metadata observed on the same opened object.
    ///
    /// # Parameters
    /// - `info`: Identity and metadata captured from the object that was opened.
    /// - `session`: Session retaining that same object or immutable version.
    ///
    /// # Returns
    /// The provider envelope for facade validation.
    pub fn new(info: OpenedFileInfo, session: Box<dyn AsyncRandomAccessReadSession>) -> Self {
        Self { info, session }
    }
    /// Returns provider-reported identity and open-time metadata.
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn AsyncRandomAccessReadSession>) {
        (self.info, self.session)
    }
}
