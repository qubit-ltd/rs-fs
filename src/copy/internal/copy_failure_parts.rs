//! Private storage for synchronous copy failure details.
use crate::copy::CopyFailureState;
use crate::copy::CopyStats;
use crate::error::FsError;
use crate::write::FileWriter;
pub(in crate::copy) struct CopyFailureParts {
    pub(in crate::copy) error: FsError,
    pub(in crate::copy) state: CopyFailureState,
    pub(in crate::copy) partial_stats: CopyStats,
    pub(in crate::copy) writer: Option<Box<FileWriter>>,
}
