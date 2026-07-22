// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Convenience extension methods for [`crate::AsyncFileSystem`].

use std::io::ErrorKind as IoErrorKind;

use qubit_io::{
    AsyncInput,
    AsyncOutput,
};

use crate::{
    AsyncFileSystem,
    FsError,
    FsErrorKind,
    FsFuture,
    FsOperation,
    FsPath,
    ReadOptions,
    WriteOptions,
    WriteOutcome,
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
    fn read_all_async<'a>(
        &'a self,
        path: &'a FsPath,
        max_bytes: usize,
    ) -> FsFuture<'a, Vec<u8>>;

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
    fn read_all_async<'a>(
        &'a self,
        path: &'a FsPath,
        max_bytes: usize,
    ) -> FsFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let mut reader =
                self.open_reader_async(path, ReadOptions::default()).await?;
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 8192];
            while bytes.len() < max_bytes {
                let remaining = max_bytes - bytes.len();
                let read_limit = remaining.min(buffer.len());
                match reader.read_async(&mut buffer[..read_limit]).await {
                    Ok(0) => return Ok(bytes),
                    Ok(read) => bytes.extend_from_slice(&buffer[..read]),
                    Err(error) if error.kind() == IoErrorKind::Interrupted => {}
                    Err(error) => return Err(async_read_error(path, error)),
                }
            }

            let mut probe = [0_u8; 1];
            loop {
                match reader.read_async(&mut probe).await {
                    Ok(0) => return Ok(bytes),
                    Ok(_) => {
                        return Err(FsError::new(
                            FsErrorKind::ResourceLimitExceeded,
                            FsOperation::Read,
                            "resource exceeds the caller byte limit",
                        )
                        .with_path(path.clone()));
                    }
                    Err(error) if error.kind() == IoErrorKind::Interrupted => {}
                    Err(error) => return Err(async_read_error(path, error)),
                }
            }
        })
    }

    fn write_all_async<'a>(
        &'a self,
        path: &'a FsPath,
        bytes: &'a [u8],
    ) -> FsFuture<'a, WriteOutcome> {
        Box::pin(async move {
            let mut writer = self
                .open_writer_async(path, WriteOptions::default())
                .await?;
            if let Err(error) = writer.write_fully_async(bytes).await {
                let _ = writer.abort_async().await;
                return Err(FsError::with_source(
                    FsErrorKind::Io,
                    FsOperation::Write,
                    "failed to write resource",
                    error,
                )
                .with_path(path.clone()));
            }
            writer.commit_async().await
        })
    }
}

fn async_read_error(path: &FsPath, error: std::io::Error) -> FsError {
    FsError::with_source(
        FsErrorKind::Io,
        FsOperation::Read,
        "failed to read resource",
        error,
    )
    .with_path(path.clone())
}
