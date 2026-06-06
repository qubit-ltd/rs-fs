// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Convenience extension methods for [`crate::FileSystem`].

use std::io::{
    Read,
    Write,
};

use crate::{
    FileSystem,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
    FsResult,
    ReadOptions,
    WriteOptions,
    WriteOutcome,
};

/// Convenience methods for filesystem trait objects.
pub trait FileSystemExt {
    /// Reads an entire resource into memory.
    ///
    /// # Parameters
    /// - `path`: Resource path.
    ///
    /// # Returns
    /// Resource bytes.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when opening or reading fails.
    fn read_all(&self, path: &FsPath) -> FsResult<Vec<u8>>;

    /// Writes an entire resource and commits the writer.
    ///
    /// # Parameters
    /// - `path`: Resource path.
    /// - `bytes`: Bytes to write.
    ///
    /// # Returns
    /// Write outcome.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when opening, writing, or committing fails.
    fn write_all(&self, path: &FsPath, bytes: &[u8]) -> FsResult<WriteOutcome>;
}

impl<T> FileSystemExt for T
where
    T: FileSystem + ?Sized,
{
    fn read_all(&self, path: &FsPath) -> FsResult<Vec<u8>> {
        let mut reader = self.open_reader(path, &ReadOptions::default())?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map_err(|error| {
            FsError::with_source(
                FsErrorKind::Io,
                FsOperation::OpenReader,
                "failed to read resource",
                error,
            )
            .with_path(path.clone())
        })?;
        Ok(bytes)
    }

    fn write_all(&self, path: &FsPath, bytes: &[u8]) -> FsResult<WriteOutcome> {
        let mut writer = self.open_writer(path, &WriteOptions::default())?;
        if let Err(error) = writer.write_all(bytes) {
            let _ = writer.abort();
            return Err(FsError::with_source(
                FsErrorKind::Io,
                FsOperation::OpenWriter,
                "failed to write resource",
                error,
            )
            .with_path(path.clone()));
        }
        writer.commit()
    }
}
