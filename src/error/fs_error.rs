/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Concrete filesystem error type.

use std::error::Error;
use std::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};

use crate::{
    FsErrorKind,
    FsOperation,
    FsPath,
};

/// Provider-neutral filesystem error with operation and path context.
#[derive(Debug)]
pub struct FsError {
    /// Error category.
    pub kind: FsErrorKind,
    /// Operation that produced the error.
    pub operation: FsOperation,
    /// Primary path involved in the operation.
    pub path: Option<Box<FsPath>>,
    /// Secondary path involved in the operation.
    pub target: Option<Box<FsPath>>,
    /// Provider id or alias involved in the operation.
    pub provider: Option<Box<str>>,
    /// Human-readable error message.
    pub message: Box<str>,
    /// Lower-level source error.
    pub source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl FsError {
    /// Creates a filesystem error without path or provider context.
    ///
    /// # Parameters
    /// - `kind`: Provider-neutral error category.
    /// - `operation`: Operation that produced the error.
    /// - `message`: Human-readable diagnostic message.
    ///
    /// # Returns
    /// New filesystem error.
    #[inline]
    pub fn new(kind: FsErrorKind, operation: FsOperation, message: &str) -> Self {
        Self {
            kind,
            operation,
            path: None,
            target: None,
            provider: None,
            message: message.into(),
            source: None,
        }
    }

    /// Creates a filesystem error that wraps a lower-level source error.
    ///
    /// # Parameters
    /// - `kind`: Provider-neutral error category.
    /// - `operation`: Operation that produced the error.
    /// - `message`: Human-readable diagnostic message.
    /// - `source`: Lower-level error to preserve.
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
    /// - `provider`: Provider id or alias involved in the operation.
    ///
    /// # Returns
    /// Updated filesystem error.
    #[inline]
    #[must_use]
    pub fn with_provider(mut self, provider: &str) -> Self {
        self.provider = Some(provider.into());
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
