//! Provider-opened random-access reader envelope.
use super::RandomAccessReadSession;
use crate::metadata::OpenedFileInfo;

/// Opened object metadata and its bound random-access session.
pub struct OpenedRandomAccessReader {
    info: OpenedFileInfo,
    session: Box<dyn RandomAccessReadSession>,
}
impl OpenedRandomAccessReader {
    /// Creates an envelope from metadata observed on the same opened object.
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
