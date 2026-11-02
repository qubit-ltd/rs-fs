//! Provider-opened random-access reader envelope.
use super::RandomAccessReadSession;
use crate::metadata::OpenedFileInfo;

/// Opened object metadata and its bound random-access session.
///
/// The metadata must describe the same opened object as the session. In
/// particular, do not perform a second path lookup to fill in its length.
///
/// # Examples
///
/// ```
/// use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::{OpenedRandomAccessReader, RandomAccessReadSession};
/// use qubit_fs::{FsResult, Path as FsPath};
///
/// struct Empty;
/// impl RandomAccessReadSession for Empty {
///     fn read_at(&mut self, _: u64, _: &mut [u8]) -> FsResult<usize> { Ok(0) }
/// }
/// let info = OpenedFileInfo::new(FileSystemId::new("fixture")?, FsPath::parse("/x")?)
///     .with_metadata(FileMetadata::new(FileKind::File).with_len(Some(0)));
/// let _opened = OpenedRandomAccessReader::new(info, Box::new(Empty));
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
pub struct OpenedRandomAccessReader {
    info: OpenedFileInfo,
    session: Box<dyn RandomAccessReadSession>,
}
impl OpenedRandomAccessReader {
    /// Creates an envelope from metadata observed on the same opened object.
    ///
    /// # Parameters
    /// - `info`: Identity and metadata captured from the object that was opened.
    /// - `session`: Session retaining that same object or immutable version.
    ///
    /// # Returns
    /// The provider envelope for facade validation.
    pub fn new(info: OpenedFileInfo, session: Box<dyn RandomAccessReadSession>) -> Self {
        Self { info, session }
    }
    /// Returns provider-reported identity and open-time metadata.
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn RandomAccessReadSession>) {
        (self.info, self.session)
    }
}
