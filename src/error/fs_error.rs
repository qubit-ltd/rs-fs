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

use crate::{
    FileSystemCapability,
    FsErrorKind,
    FsOperation,
    Path,
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
    path: Option<Box<Path>>,
    /// Secondary path involved in the operation.
    target: Option<Box<Path>>,
    /// Provider id or alias involved in the operation.
    provider: Option<Box<str>>,
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
    pub fn with_path(mut self, path: impl Into<Path>) -> Self {
        self.path = Some(Box::new(path.into()));
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
    pub fn with_target(mut self, target: impl Into<Path>) -> Self {
        self.target = Some(Box::new(target.into()));
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
    pub fn with_provider(mut self, provider: impl Display) -> Self {
        self.provider = Some(provider.to_string().into());
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

    /// Adds missing path, target, and provider context without overwriting
    /// provider-supplied details.
    ///
    /// Core resource wrappers use this when an error crosses an abstraction
    /// boundary. It preserves a provider's more specific context while making
    /// generic validation and stream failures actionable to callers.
    ///
    /// # Parameters
    /// - `path`: Fallback primary path for the requested operation.
    /// - `target`: Fallback secondary path, when the operation has one.
    /// - `provider`: Fallback canonical provider id.
    ///
    /// # Returns
    /// Updated error with every previously absent context field filled.
    #[inline]
    #[must_use]
    pub(crate) fn with_missing_context(
        mut self,
        path: &Path,
        target: Option<&Path>,
        provider: &str,
    ) -> Self {
        if self.path.is_none() {
            self.path = Some(Box::new(path.clone()));
        }
        if self.target.is_none() {
            self.target = target.cloned().map(Box::new);
        }
        if self.provider.is_none() {
            self.provider = Some(provider.into());
        }
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
            io::ErrorKind::DirectoryNotEmpty => FsErrorKind::Conflict,
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

    /// Restores a filesystem error transported through an I/O boundary.
    ///
    /// Provider streams may embed an [`FsError`] inside [`io::Error`]. This
    /// helper recovers that typed error when present; ordinary I/O errors use
    /// [`Self::from_io`] classification instead. An untyped `InvalidData`
    /// remains generic I/O because stream adapters also use it for contract
    /// violations; providers report verified corruption with an embedded typed
    /// error.
    ///
    /// # Parameters
    ///
    /// * `error` - Stream error returned by a reader or writer.
    /// * `operation` - Public filesystem operation consuming the stream.
    /// * `path` - Resource path supplied to that public operation.
    ///
    /// # Returns
    ///
    /// A typed filesystem error with the public operation and path rebound.
    #[allow(dead_code)]
    #[inline]
    pub(crate) fn from_stream_io(
        error: io::Error,
        operation: FsOperation,
        path: &Path,
    ) -> Self {
        match error.downcast::<Self>() {
            Ok(error) => {
                error.with_operation(operation).with_path(path.clone())
            }
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                Self::with_source(
                    FsErrorKind::Io,
                    operation,
                    "stream I/O contract failed",
                    error,
                )
                .with_path(path.clone())
            }
            Err(error) => {
                Self::from_io(error, operation).with_path(path.clone())
            }
        }
    }

    /// Gets the error kind.
    ///
    /// # Returns
    /// Error category.
    #[inline(always)]
    #[must_use]
    pub fn kind(&self) -> FsErrorKind {
        self.kind
    }

    /// Gets the operation that produced this error.
    ///
    /// # Returns
    /// The provider-neutral operation identifier.
    #[inline(always)]
    #[must_use]
    pub fn operation(&self) -> FsOperation {
        self.operation
    }

    /// Gets the primary path associated with this error.
    ///
    /// # Returns
    /// The path when one was attached.
    #[inline(always)]
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Gets the secondary target path associated with this error.
    ///
    /// # Returns
    /// The target path when one was attached.
    #[inline(always)]
    #[must_use]
    pub fn target(&self) -> Option<&Path> {
        self.target.as_deref()
    }

    /// Gets the provider associated with this error.
    ///
    /// # Returns
    /// The canonical provider id when one was attached.
    #[inline(always)]
    #[must_use]
    pub fn provider(&self) -> Option<&str> {
        self.provider.as_deref()
    }

    /// Gets the required capability associated with this error.
    ///
    /// # Returns
    /// The capability when the error describes unsupported functionality or
    /// an unmet semantic requirement.
    #[inline(always)]
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
