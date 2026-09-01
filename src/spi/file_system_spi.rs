// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Synchronous provider implementation contract.

use super::CopyAttempt;
use super::CopyDeclineReason;
use super::CopyRequest;
use super::CreateDirectoryRequest;
use super::CreateTempDirectoryRequest;
use super::CreateTempFileRequest;
use super::DeleteDirectoryRequest;
use super::DeleteFileRequest;
use super::ListRequest;
use super::OpenReaderRequest;
use super::OpenWriterRequest;
use super::OpenedDirectoryStream;
use super::OpenedReader;
use super::OpenedTempDirectory;
use super::OpenedTempFile;
use super::OpenedWriter;
use super::ProviderProperties;
use super::RenameRequest;
use super::SpiCopyFailure;
use super::SpiRenameFailure;
use super::StatRequest;
use super::StatResponse;
use crate::directory::CreateDirectoryOutcome;
use crate::directory::DeleteOutcome;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::rename::RenameFailureState;
use crate::rename::RenameOutcome;

/// Synchronous provider implementation contract.
pub trait FileSystemSpi: Send + Sync {
    /// Returns one immutable property snapshot.
    ///
    /// # Returns
    /// The provider's immutable property snapshot.
    fn properties(&self) -> ProviderProperties;
    /// Reads metadata for a validated request.
    ///
    /// # Parameters
    /// - `request`: Facade-validated stat request.
    ///
    /// # Returns
    /// Path-bound provider metadata.
    ///
    /// # Errors
    /// Returns the provider lookup failure with filesystem context.
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse>;
    /// Opens a directory stream.
    ///
    /// # Parameters
    /// - `request`: Facade-validated list request.
    ///
    /// # Returns
    /// An opened provider enumeration session.
    ///
    /// # Errors
    /// Returns the provider open failure with filesystem context.
    fn list(&self, request: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
        Err(unsupported(FsOperation::List, request.path()))
    }
    /// Opens a reader.
    ///
    /// # Parameters
    /// - `request`: Facade-validated reader request.
    ///
    /// # Returns
    /// An identity-bound provider reader.
    ///
    /// # Errors
    /// Returns the provider open failure with filesystem context.
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Err(unsupported(FsOperation::OpenReader, request.path()))
    }
    /// Opens a writer.
    ///
    /// # Parameters
    /// - `request`: Facade-validated writer request.
    ///
    /// # Returns
    /// An identity-bound provider writer.
    ///
    /// # Errors
    /// Returns the provider open failure with filesystem context.
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Err(unsupported(FsOperation::OpenWriter, request.path()))
    }
    /// Creates a directory.
    ///
    /// # Parameters
    /// - `request`: Facade-validated directory-creation request.
    ///
    /// # Returns
    /// The confirmed creation outcome.
    ///
    /// # Errors
    /// Returns the provider creation failure with filesystem context.
    fn create_directory(&self, request: CreateDirectoryRequest<'_>) -> FsResult<CreateDirectoryOutcome> {
        Err(unsupported(FsOperation::CreateDir, request.path()))
    }
    /// Deletes a file.
    ///
    /// # Parameters
    /// - `request`: Facade-validated file-deletion request.
    ///
    /// # Returns
    /// The confirmed deletion outcome.
    ///
    /// # Errors
    /// Returns the provider deletion failure with filesystem context.
    fn delete_file(&self, request: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(unsupported(FsOperation::Delete, request.path()))
    }
    /// Deletes a directory.
    ///
    /// # Parameters
    /// - `request`: Facade-validated directory-deletion request.
    ///
    /// # Returns
    /// The confirmed deletion outcome.
    ///
    /// # Errors
    /// Returns the provider deletion failure with filesystem context.
    fn delete_directory(&self, request: DeleteDirectoryRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(unsupported(FsOperation::Delete, request.path()))
    }
    /// Attempts an optional provider copy primitive.
    ///
    /// # Parameters
    /// - `_request`: Facade-validated copy request.
    ///
    /// # Returns
    /// A completed outcome or a typed decline reason.
    ///
    /// # Errors
    /// Returns a typed failure preserving confirmed publication progress.
    #[inline(always)]
    fn try_copy(&self, _request: CopyRequest<'_>) -> Result<CopyAttempt, SpiCopyFailure> {
        Ok(CopyAttempt::Declined(CopyDeclineReason::NotImplemented))
    }
    /// Renames a resource.
    ///
    /// # Parameters
    /// - `request`: Facade-validated rename request.
    ///
    /// # Returns
    /// The confirmed rename outcome.
    ///
    /// # Errors
    /// Returns a typed failure preserving confirmed rename progress.
    fn rename(&self, request: RenameRequest<'_>) -> Result<RenameOutcome, SpiRenameFailure> {
        Err(SpiRenameFailure::new(
            unsupported(FsOperation::Rename, request.source()).with_target(request.target().clone()),
            RenameFailureState::Unchanged,
        ))
    }
    /// Creates a temporary file.
    ///
    /// # Parameters
    /// - `request`: Validated temporary-file creation request.
    ///
    /// # Returns
    /// An identity-bound temporary-file session.
    ///
    /// # Errors
    /// Returns the provider creation failure with filesystem context.
    fn create_temp_file(&self, _request: CreateTempFileRequest) -> FsResult<OpenedTempFile> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::CreateTemp,
            "provider does not implement this operation",
        ))
    }
    /// Creates a temporary directory.
    ///
    /// # Parameters
    /// - `request`: Validated temporary-directory creation request.
    ///
    /// # Returns
    /// An identity-bound temporary-directory session.
    ///
    /// # Errors
    /// Returns the provider creation failure with filesystem context.
    fn create_temp_directory(&self, _request: CreateTempDirectoryRequest) -> FsResult<OpenedTempDirectory> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::CreateTemp,
            "provider does not implement this operation",
        ))
    }
}

/// Builds a standard unsupported-operation error for a validated path request.
fn unsupported(operation: FsOperation, path: &crate::path::Path) -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        operation,
        "provider does not implement this operation",
    )
    .with_path(path.clone())
}
