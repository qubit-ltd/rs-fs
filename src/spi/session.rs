// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider sessions adapted by concrete facade handles.

use qubit_io::Output;

use crate::{
    DirEntry,
    FsError,
    FsResult,
    PersistFailureState,
    PersistOutcome,
    WriteFailureState,
    WriteOutcome,
};

use super::PersistRequest;

/// Typed provider write failure preserving recovery state.
pub struct SpiWriteFailure {
    error: FsError,
    state: WriteFailureState,
}
impl SpiWriteFailure {
    /// Creates a typed provider write failure.
    pub fn new(error: FsError, state: WriteFailureState) -> Self {
        Self { error, state }
    }
    /// Returns the underlying error.
    pub const fn error(&self) -> &FsError {
        &self.error
    }
    /// Returns confirmed publication state.
    pub const fn state(&self) -> WriteFailureState {
        self.state
    }
    /// Returns owned failure parts.
    pub fn into_parts(self) -> (FsError, WriteFailureState) {
        (self.error, self.state)
    }
}
/// Provider writer session.
pub trait FileWriterSpi: Output<Item = u8> + Send {
    /// Publishes accepted bytes.
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure>;
    /// Releases provider staging resources.
    fn abort(&mut self) -> FsResult<()>;
}
/// Provider directory enumeration session.
pub trait DirectoryStreamSpi: Send {
    /// Returns the next lazy directory entry.
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>>;
}
/// Typed provider persist failure preserving partial publication state.
pub struct SpiPersistFailure {
    error: FsError,
    state: PersistFailureState,
}
impl SpiPersistFailure {
    /// Creates a typed provider persist failure.
    pub fn new(error: FsError, state: PersistFailureState) -> Self {
        Self { error, state }
    }
    /// Returns the underlying error.
    pub const fn error(&self) -> &FsError {
        &self.error
    }
    /// Returns confirmed persistence state.
    pub const fn state(&self) -> PersistFailureState {
        self.state
    }
    /// Returns owned failure parts.
    pub fn into_parts(self) -> (FsError, PersistFailureState) {
        (self.error, self.state)
    }
}
/// Provider temporary-resource lifecycle session.
pub trait TempResourceSpi: Send {
    /// Persists a temporary resource.
    fn persist(
        &mut self,
        request: PersistRequest<'_>,
    ) -> Result<PersistOutcome, SpiPersistFailure>;
    /// Transfers source ownership to the caller.
    fn keep(&mut self) -> FsResult<()>;
    /// Cleans the temporary source.
    fn cleanup(&mut self) -> FsResult<()>;
}
