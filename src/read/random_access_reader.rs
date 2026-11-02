//! Application-facing handle for bounded offset reads.
use crate::error::{FsError, FsErrorKind, FsOperation, FsResult};
use crate::metadata::{FileSystemLimit, OpenedFileInfo};
use crate::read::RandomAccessReadPolicy;
use crate::spi::RandomAccessReadSession;
use std::fmt::{Debug, Formatter, Result as FmtResult};

/// A random-access session bound to one opened object and its open-time length.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::{FileSystem, FsResult, Path};
/// use qubit_fs::read::{RandomAccessReader, ReadOptions};
///
/// fn sample(filesystem: &FileSystem, path: &Path) -> FsResult<Vec<u8>> {
///     let mut reader: RandomAccessReader = filesystem.open_random_access_reader(path, ReadOptions::default())?;
///     let mut bytes = [0; 16];
///     let count = reader.read_at(0, &mut bytes)?;
///     Ok(bytes[..count].to_vec())
/// }
/// ```
pub struct RandomAccessReader {
    info: OpenedFileInfo,
    len: u64,
    limit: FileSystemLimit,
    provider_id: String,
    session: Box<dyn RandomAccessReadSession>,
}
impl RandomAccessReader {
    pub(crate) fn new(
        info: OpenedFileInfo,
        len: u64,
        limit: FileSystemLimit,
        provider_id: String,
        session: Box<dyn RandomAccessReadSession>,
    ) -> Self {
        Self {
            info,
            len,
            limit,
            provider_id,
            session,
        }
    }
    /// Returns open-time identity and metadata.
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }
    /// Returns the accessible upper bound observed when opened.
    pub fn len(&self) -> u64 {
        self.len
    }
    /// Returns whether the open-time length is zero.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Reads into caller storage at an explicit offset. Requests past `len` are clipped.
    pub fn read_at(&mut self, offset: u64, output: &mut [u8]) -> FsResult<usize> {
        let count =
            RandomAccessReadPolicy::validate_read(offset, output.len(), self.len, self.limit)
                .map_err(|e| {
                    e.with_path(self.info.path().clone())
                        .with_provider(&self.provider_id)
                })?;
        if count == 0 {
            return Ok(0);
        }
        let n = self
            .session
            .read_at(offset, &mut output[..count])
            .map_err(|e| {
                e.with_path(self.info.path().clone())
                    .with_provider(&self.provider_id)
                    .with_operation(FsOperation::Read)
            })?;
        if n > count {
            return Err(FsError::new(
                FsErrorKind::ProviderContractViolation,
                FsOperation::Read,
                "provider returned more bytes than supplied buffer",
            )
            .with_path(self.info.path().clone())
            .with_provider(&self.provider_id));
        }
        Ok(n)
    }
}
impl Debug for RandomAccessReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("RandomAccessReader")
            .field("info", &self.info)
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}
