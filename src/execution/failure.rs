use crate::{
    copy::CopyFailure,
    error::{FsEffectState, FsError},
    rename::RenameFailure,
    write::WriteAllFailure,
};
use qubit_operation::AdmissionError;
use std::{error::Error, fmt, sync::Arc};

/// Typed failure produced by a managed filesystem operation.
#[derive(Debug)]
pub enum FsOperationFailureKind {
    /// General provider or read failure retaining its structured filesystem error.
    Provider(Arc<FsError>),
    /// Copy failure retaining progress and any typed recovery owner.
    Copy(Box<CopyFailure>),
    /// Write failure retaining progress and publication facts.
    Write(Box<WriteAllFailure>),
    /// Rename failure retaining publication facts.
    Rename(Box<RenameFailure>),
}

/// Filesystem failure with the strongest known storage effect.
#[derive(Debug)]
pub struct FsOperationFailure {
    /// Structured operation-specific failure.
    pub kind: FsOperationFailureKind,
    /// Strongest known storage effect when the operation failed.
    pub effect: FsEffectState,
}

impl FsOperationFailure {
    /// Creates a typed failure with a known storage effect.
    pub fn new(kind: FsOperationFailureKind, effect: FsEffectState) -> Self {
        Self { kind, effect }
    }
}
impl fmt::Display for FsOperationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            FsOperationFailureKind::Provider(error) => error.fmt(f),
            FsOperationFailureKind::Copy(error) => error.fmt(f),
            FsOperationFailureKind::Write(error) => error.fmt(f),
            FsOperationFailureKind::Rename(error) => error.fmt(f),
        }
    }
}
impl Error for FsOperationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            FsOperationFailureKind::Provider(error) => Some(error.as_ref()),
            FsOperationFailureKind::Copy(error) => Some(error.as_ref()),
            FsOperationFailureKind::Write(error) => Some(error.as_ref()),
            FsOperationFailureKind::Rename(error) => Some(error.as_ref()),
        }
    }
}

/// Reason a provider could not prepare an owned request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FsPrepareFailureKind {
    /// Operation is unsupported.
    Unsupported,
    /// Request was rejected before I/O with a bounded diagnostic.
    Rejected(String),
    /// No bounded mutation fact slot was available before preparation.
    EffectCapacity,
    /// Provider did not accept its reserved fact slot before submission.
    ReservationInstallFailed,
}
/// Preparation failure retaining the exact unclaimed request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FsPrepareFailure<R> {
    /// Preparation reason.
    pub kind: FsPrepareFailureKind,
    /// Original owned request.
    pub request: R,
}
impl<R> FsPrepareFailure<R> {
    /// Returns the reason and original request.
    pub fn into_parts(self) -> (FsPrepareFailureKind, R) {
        (self.kind, self.request)
    }
}
/// Submission failure retaining the original owned request.
#[derive(Debug)]
pub struct FsSubmitFailure<R> {
    /// Admission error, if submission was attempted.
    pub admission: Option<AdmissionError>,
    /// Preparation or binding error, if preparation failed.
    pub preparation: Option<FsPrepareFailureKind>,
    /// Original owned request.
    pub request: R,
}
impl<R> FsSubmitFailure<R> {
    /// Returns failure details and original request.
    pub fn into_parts(self) -> (Option<AdmissionError>, Option<FsPrepareFailureKind>, R) {
        (self.admission, self.preparation, self.request)
    }
}
