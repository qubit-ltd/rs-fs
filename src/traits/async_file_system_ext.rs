// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Convenience extension methods for [`crate::AsyncFileSystem`].

use qubit_io::{AsyncInput, AsyncOutput};

use crate::{
    AsyncFileReader, AsyncFileSystem, AsyncFileWriter, FsError, FsErrorKind, FsFuture, FsOperation,
    FsPath, ReadOptions, WriteOptions, WriteOutcome,
};

/// Future-based convenience methods for asynchronous filesystem objects.
pub trait AsyncFileSystemExt {
    /// Reads an entire resource into memory asynchronously.
    ///
    /// # Parameters
    ///
    /// - `path`: Provider-local resource path.
    /// - `max_bytes`: Maximum number of bytes to retain in memory.
    ///
    /// # Returns
    ///
    /// A future resolving to the complete resource bytes.
    ///
    /// # Errors
    ///
    /// The future resolves to an error when opening or reading fails, or when
    /// the resource contains more than `max_bytes` bytes.
    fn read_all_async<'a>(&'a self, path: &'a FsPath, max_bytes: usize) -> FsFuture<'a, Vec<u8>>;

    /// Writes all bytes and commits the asynchronous writer.
    ///
    /// If byte transfer fails, this helper attempts an explicit asynchronous
    /// abort before returning the transfer error. Abort failure cannot replace
    /// the primary error; callers that need both diagnostics should use the
    /// writer lifecycle directly.
    ///
    /// # Parameters
    ///
    /// - `path`: Provider-local destination path.
    /// - `bytes`: Complete byte content to write.
    ///
    /// # Returns
    ///
    /// A future resolving to the provider's publication outcome.
    ///
    /// # Errors
    ///
    /// The future resolves to an error when opening, writing, aborting, or
    /// committing fails, or when a declared finite provider limit is exceeded.
    fn write_all_async<'a>(
        &'a self,
        path: &'a FsPath,
        bytes: &'a [u8],
    ) -> FsFuture<'a, WriteOutcome>;
}

impl<T> AsyncFileSystemExt for T
where
    T: AsyncFileSystem + ?Sized,
{
    fn read_all_async<'a>(&'a self, path: &'a FsPath, max_bytes: usize) -> FsFuture<'a, Vec<u8>> {
        Box::pin(async move {
            self.limits()
                .validate_path(path, self.info().path_semantics(), FsOperation::Read)?;
            let reader = self.open_reader_async(path, ReadOptions::default()).await?;
            read_all_from_async(reader, path, max_bytes).await
        })
    }

    fn write_all_async<'a>(
        &'a self,
        path: &'a FsPath,
        bytes: &'a [u8],
    ) -> FsFuture<'a, WriteOutcome> {
        Box::pin(async move {
            self.limits()
                .validate_path(path, self.info().path_semantics(), FsOperation::Write)?;
            self.limits().validate_write_size(path, bytes.len())?;
            let writer = self
                .open_writer_async(path, WriteOptions::default())
                .await?;
            write_all_to_async(writer, path, bytes).await
        })
    }
}

/// Reads all bytes from an opened asynchronous reader within `max_bytes`.
///
/// Returns a read error with `path` context, or a resource-limit error when a
/// one-byte probe finds content beyond the caller budget.
async fn read_all_from_async(
    mut reader: AsyncFileReader,
    path: &FsPath,
    max_bytes: usize,
) -> Result<Vec<u8>, FsError> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    while bytes.len() < max_bytes {
        let remaining = max_bytes - bytes.len();
        let read_limit = remaining.min(buffer.len());
        match reader.read_async(&mut buffer[..read_limit]).await {
            Ok(0) => return Ok(bytes),
            Ok(read) => bytes.extend_from_slice(&buffer[..read]),
            Err(error) => return Err(async_read_error(path, error)),
        }
    }

    let mut probe = [0_u8; 1];
    match reader.read_async(&mut probe).await {
        Ok(0) => Ok(bytes),
        Ok(_) => Err(FsError::new(
            FsErrorKind::ResourceLimitExceeded,
            FsOperation::Read,
            "resource exceeds the caller byte limit",
        )
        .with_path(path.clone())),
        Err(error) => Err(async_read_error(path, error)),
    }
}

/// Writes `bytes` to an opened asynchronous writer and commits it.
///
/// On transfer failure this function attempts a best-effort asynchronous
/// abort, then returns the transfer error with `path` context.
async fn write_all_to_async(
    mut writer: AsyncFileWriter,
    path: &FsPath,
    bytes: &[u8],
) -> Result<WriteOutcome, FsError> {
    if let Err(error) = writer.write_fully_async(bytes).await {
        let _ = writer.abort_async().await;
        return Err(FsError::from_stream_io(error, FsOperation::Write, path));
    }
    writer.commit_async().await
}

/// Adds filesystem read context to an asynchronous stream error.
fn async_read_error(path: &FsPath, error: std::io::Error) -> FsError {
    FsError::from_stream_io(error, FsOperation::Read, path)
}
