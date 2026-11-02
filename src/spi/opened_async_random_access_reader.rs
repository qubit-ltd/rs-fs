//! Provider-opened asynchronous random-access reader envelope.
use super::AsyncRandomAccessReadSession;
use crate::metadata::OpenedFileInfo;

/// Opened object metadata and its bound asynchronous session.
pub struct OpenedAsyncRandomAccessReader {
    info: OpenedFileInfo,
    session: Box<dyn AsyncRandomAccessReadSession>,
}
impl OpenedAsyncRandomAccessReader {
    /// Creates an envelope from metadata observed on the same opened object.
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
