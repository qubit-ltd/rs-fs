// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Convenience extension methods for [`crate::FileSystem`].

use std::io::ErrorKind as IoErrorKind;

use qubit_io::{Input, Output};

use crate::{
    FileReader, FileSystem, FileWriter, FsError, FsErrorKind, FsOperation, FsPath, FsResult,
    ReadOptions, WriteOptions, WriteOutcome,
};

/// Convenience methods for filesystem trait objects.
pub trait FileSystemExt {
    /// Reads an entire resource into memory.
    ///
    /// # Parameters
    /// - `path`: Resource path.
    /// - `max_bytes`: Maximum number of bytes to retain in memory.
    ///
    /// # Returns
    /// Resource bytes.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when opening or reading fails, or when the
    /// resource contains more than `max_bytes` bytes.
    fn read_all(&self, path: &FsPath, max_bytes: usize) -> FsResult<Vec<u8>>;

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
    fn read_all(&self, path: &FsPath, max_bytes: usize) -> FsResult<Vec<u8>> {
        self.limits()
            .validate_path(path, self.info().path_semantics(), FsOperation::Read)?;
        let reader = self.open_reader(path, ReadOptions::default())?;
        read_all_from(reader, path, max_bytes)
    }

    fn write_all(&self, path: &FsPath, bytes: &[u8]) -> FsResult<WriteOutcome> {
        self.limits()
            .validate_path(path, self.info().path_semantics(), FsOperation::Write)?;
        self.limits().validate_write_size(path, bytes.len())?;
        let writer = self.open_writer(path, WriteOptions::default())?;
        write_all_to(writer, path, bytes)
    }
}

/// Reads all bytes from an opened reader within `max_bytes`.
///
/// Retries interrupted synchronous reads and returns a resource-limit error
/// when a one-byte probe finds content beyond the caller budget.
fn read_all_from(mut reader: FileReader, path: &FsPath, max_bytes: usize) -> FsResult<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    while bytes.len() < max_bytes {
        let remaining = max_bytes - bytes.len();
        let read_limit = remaining.min(buffer.len());
        match reader.read(&mut buffer[..read_limit]) {
            Ok(0) => return Ok(bytes),
            Ok(read) => bytes.extend_from_slice(&buffer[..read]),
            Err(error) if error.kind() == IoErrorKind::Interrupted => {}
            Err(error) => return Err(read_error(path, error)),
        }
    }

    let mut probe = [0_u8; 1];
    loop {
        match reader.read(&mut probe) {
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
            Err(error) => return Err(read_error(path, error)),
        }
    }
}

/// Writes `bytes` to an opened writer and commits it.
///
/// On transfer failure this function attempts a best-effort abort, then
/// returns the transfer error with `path` context.
fn write_all_to(mut writer: FileWriter, path: &FsPath, bytes: &[u8]) -> FsResult<WriteOutcome> {
    if let Err(error) = writer.write_fully(bytes) {
        let _ = writer.abort();
        return Err(FsError::from_stream_io(error, FsOperation::Write, path));
    }
    writer.commit()
}

/// Adds filesystem read context to a synchronous stream error.
fn read_error(path: &FsPath, error: std::io::Error) -> FsError {
    FsError::from_stream_io(error, FsOperation::Read, path)
}
