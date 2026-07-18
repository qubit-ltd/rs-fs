// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete filesystem error type.

use std::error::Error;
use std::fmt::{
    Debug,
    Display,
    Formatter,
    Result as FmtResult,
};
use std::io;

use qubit_spi::ProviderId;

use crate::{
    FileSystemCapability,
    FsErrorKind,
    FsOperation,
    FsPath,
};

/// Provider-neutral filesystem error with operation and path context.
///
/// [`Debug`] and [`Display`] never expand the retained source error because a
/// lower-level SDK or transport diagnostic may contain credentials. `Debug`
/// reports only whether a source exists; explicit diagnostic code may inspect
/// it through [`Error::source`]. The message supplied by constructors must
/// already be scrubbed of secret material.
pub struct FsError {
    /// Error category.
    kind: FsErrorKind,
    /// Operation that produced the error.
    operation: FsOperation,
    /// Primary path involved in the operation.
    path: Option<Box<FsPath>>,
    /// Secondary path involved in the operation.
    target: Option<Box<FsPath>>,
    /// Provider id or alias involved in the operation.
    provider: Option<ProviderId>,
    /// Capability needed to satisfy the request, when applicable.
    required_capability: Option<FileSystemCapability>,
    /// Human-readable, non-sensitive error message.
    message: Box<str>,
    /// Lower-level source error, excluded from automatic formatting.
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl FsError {
    /// Creates a filesystem error without path or provider context.
    ///
    /// # Parameters
    /// - `kind`: Provider-neutral error category.
    /// - `operation`: Operation that produced the error.
    /// - `message`: Human-readable diagnostic message that must not contain
    ///   credentials or other secret material.
    ///
    /// # Returns
    /// New filesystem error.
    #[inline]
    pub fn new(
        kind: FsErrorKind,
        operation: FsOperation,
        message: &str,
    ) -> Self {
        Self {
            kind,
            operation,
            path: None,
            target: None,
            provider: None,
            required_capability: None,
            message: message.into(),
            source: None,
        }
    }

    /// Creates a filesystem error that wraps a lower-level source error.
    ///
    /// # Parameters
    /// - `kind`: Provider-neutral error category.
    /// - `operation`: Operation that produced the error.
    /// - `message`: Human-readable diagnostic message that must not contain
    ///   credentials or other secret material.
    /// - `source`: Lower-level error to preserve. Its formatting may contain
    ///   secrets and is therefore never expanded by this type's `Debug` or
    ///   `Display` implementation.
    ///
    /// # Returns
    /// New filesystem error with source context.
    #[inline]
    pub fn with_source<E>(
        kind: FsErrorKind,
        operation: FsOperation,
        message: &str,
        source: E,
    ) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            source: Some(Box::new(source)),
            ..Self::new(kind, operation, message)
        }
    }

    /// Adds primary path context.
    ///
    /// # Parameters
    /// - `path`: Primary path involved in the operation.
    ///
    /// # Returns
    /// Updated filesystem error.
    #[inline]
    #[must_use]
    pub fn with_path(mut self, path: FsPath) -> Self {
        self.path = Some(Box::new(path));
        self
    }

    /// Rebinds the error to the public operation that was requested.
    ///
    /// This is useful for convenience operations implemented through another
    /// primitive, such as `exists` implemented through `stat`, while retaining
    /// all path, provider, capability, and source context.
    ///
    /// # Parameters
    /// - `operation`: Public operation whose failure is being returned.
    ///
    /// # Returns
    /// Updated filesystem error.
    #[inline]
    #[must_use]
    pub fn with_operation(mut self, operation: FsOperation) -> Self {
        self.operation = operation;
        self
    }

    /// Adds secondary target path context.
    ///
    /// # Parameters
    /// - `target`: Secondary path involved in the operation.
    ///
    /// # Returns
    /// Updated filesystem error.
    #[inline]
    #[must_use]
    pub fn with_target(mut self, target: FsPath) -> Self {
        self.target = Some(Box::new(target));
        self
    }

    /// Adds provider context.
    ///
    /// # Parameters
    /// - `provider`: Canonical provider id involved in the operation.
    ///
    /// # Returns
    /// Updated filesystem error.
    #[inline]
    #[must_use]
    pub fn with_provider(mut self, provider: ProviderId) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Adds the capability required by an unsupported or unmet request.
    ///
    /// # Parameters
    /// - `capability`: Stable capability required by the request.
    ///
    /// # Returns
    /// Updated filesystem error.
    #[inline]
    #[must_use]
    pub fn with_required_capability(
        mut self,
        capability: FileSystemCapability,
    ) -> Self {
        self.required_capability = Some(capability);
        self
    }

    /// Creates an invalid-path error.
    ///
    /// # Parameters
    /// - `operation`: Operation that rejected the path.
    /// - `message`: Human-readable reason.
    ///
    /// # Returns
    /// Invalid-path filesystem error.
    #[inline]
    pub fn invalid_path(operation: FsOperation, message: &str) -> Self {
        Self::new(FsErrorKind::InvalidPath, operation, message)
    }

    /// Gets the error kind.
    ///
    /// # Returns
    /// Error category.
    #[inline]
    #[must_use]
    pub fn kind(&self) -> FsErrorKind {
        self.kind
    }

    /// Gets the operation that produced this error.
    ///
    /// # Returns
    /// The provider-neutral operation identifier.
    #[inline]
    #[must_use]
    pub fn operation(&self) -> FsOperation {
        self.operation
    }

    /// Gets the primary path associated with this error.
    ///
    /// # Returns
    /// The path when one was attached.
    #[inline]
    #[must_use]
    pub fn path(&self) -> Option<&FsPath> {
        self.path.as_deref()
    }

    /// Gets the secondary target path associated with this error.
    ///
    /// # Returns
    /// The target path when one was attached.
    #[inline]
    #[must_use]
    pub fn target(&self) -> Option<&FsPath> {
        self.target.as_deref()
    }

    /// Gets the provider associated with this error.
    ///
    /// # Returns
    /// The canonical provider id when one was attached.
    #[inline]
    #[must_use]
    pub fn provider(&self) -> Option<&ProviderId> {
        self.provider.as_ref()
    }

    /// Gets the required capability associated with this error.
    ///
    /// # Returns
    /// The capability when the error describes unsupported functionality or
    /// an unmet semantic requirement.
    #[inline]
    #[must_use]
    pub fn required_capability(&self) -> Option<FileSystemCapability> {
        self.required_capability
    }

    /// Converts this filesystem error into a byte-stream error.
    ///
    /// The complete [`FsError`] is retained as the [`io::Error`] source so
    /// callers crossing the open/stream boundary do not lose provider,
    /// operation, or path context.
    ///
    /// # Returns
    /// An I/O error with a corresponding standard category.
    #[inline]
    #[must_use]
    pub fn into_io_error(self) -> io::Error {
        let kind = match self.kind {
            FsErrorKind::NotFound => io::ErrorKind::NotFound,
            FsErrorKind::AlreadyExists => io::ErrorKind::AlreadyExists,
            FsErrorKind::NotDirectory => io::ErrorKind::NotADirectory,
            FsErrorKind::IsDirectory => io::ErrorKind::IsADirectory,
            FsErrorKind::PermissionDenied
            | FsErrorKind::AuthenticationFailed => {
                io::ErrorKind::PermissionDenied
            }
            FsErrorKind::InvalidPath
            | FsErrorKind::InvalidUri
            | FsErrorKind::InvalidOptions
            | FsErrorKind::InvalidState => io::ErrorKind::InvalidInput,
            FsErrorKind::UnsupportedOperation
            | FsErrorKind::UnsupportedCapability => io::ErrorKind::Unsupported,
            FsErrorKind::Timeout => io::ErrorKind::TimedOut,
            FsErrorKind::Interrupted | FsErrorKind::Cancelled => {
                io::ErrorKind::Interrupted
            }
            FsErrorKind::QuotaExceeded => io::ErrorKind::StorageFull,
            FsErrorKind::DataCorruption => io::ErrorKind::InvalidData,
            _ => io::ErrorKind::Other,
        };
        io::Error::new(kind, self)
    }

    /// Wraps a byte-stream error with filesystem operation context.
    ///
    /// # Parameters
    /// - `error`: Lower-level stream error.
    /// - `operation`: Filesystem operation in progress when it occurred.
    ///
    /// # Returns
    /// A filesystem error retaining `error` as its source.
    #[inline]
    #[must_use]
    pub fn from_io(error: io::Error, operation: FsOperation) -> Self {
        let kind = match error.kind() {
            io::ErrorKind::NotFound => FsErrorKind::NotFound,
            io::ErrorKind::AlreadyExists => FsErrorKind::AlreadyExists,
            io::ErrorKind::NotADirectory => FsErrorKind::NotDirectory,
            io::ErrorKind::IsADirectory => FsErrorKind::IsDirectory,
            io::ErrorKind::PermissionDenied => FsErrorKind::PermissionDenied,
            io::ErrorKind::InvalidInput => FsErrorKind::InvalidOptions,
            io::ErrorKind::Unsupported => FsErrorKind::UnsupportedOperation,
            io::ErrorKind::TimedOut => FsErrorKind::Timeout,
            io::ErrorKind::Interrupted => FsErrorKind::Interrupted,
            io::ErrorKind::StorageFull => FsErrorKind::QuotaExceeded,
            io::ErrorKind::InvalidData => FsErrorKind::DataCorruption,
            _ => FsErrorKind::Io,
        };
        Self::with_source(kind, operation, "stream I/O failed", error)
    }
}

impl Debug for FsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("FsError")
            .field("kind", &self.kind)
            .field("operation", &self.operation)
            .field("path", &self.path.as_deref())
            .field("target", &self.target.as_deref())
            .field("provider", &self.provider)
            .field("required_capability", &self.required_capability)
            .field("message", &self.message)
            .field("source_present", &self.source.is_some())
            .finish()
    }
}

impl Display for FsError {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        write!(
            formatter,
            "{:?} failed with {:?}: {}",
            self.operation, self.kind, self.message,
        )
    }
}

impl Error for FsError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
