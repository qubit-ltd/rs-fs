//! URI error construction shared by URI values.

use crate::{FsError, FsErrorKind, FsOperation};

/// Builds a sanitized invalid-URI error without retaining input text.
pub(crate) fn invalid_uri(message: &'static str) -> FsError {
    FsError::new(FsErrorKind::InvalidUri, FsOperation::ParseUri, message)
}
