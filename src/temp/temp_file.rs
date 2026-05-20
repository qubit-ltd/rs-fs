/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Temporary file handle trait.

use crate::{
    FileReader,
    FileSystemExt,
    FileWriter,
    FsPath,
    FsResult,
    PersistOptions,
    ReadOptions,
    TempResource,
    WriteOptions,
    WriteOutcome,
};

/// Temporary file handle with cleanup responsibility.
pub trait TempFile: TempResource {
    /// Opens the temporary file for reading.
    ///
    /// # Parameters
    /// - `options`: Read options.
    ///
    /// # Returns
    /// Reader handle opened by the owning filesystem.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the owning filesystem cannot open the
    /// temporary file for reading.
    fn open_reader(&self, options: &ReadOptions) -> FsResult<Box<dyn FileReader>> {
        self.fs().as_ref().open_reader(self.path(), options)
    }

    /// Opens the temporary file for writing.
    ///
    /// # Parameters
    /// - `options`: Write options.
    ///
    /// # Returns
    /// Writer handle opened by the owning filesystem.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the owning filesystem cannot open the
    /// temporary file for writing.
    fn open_writer(&self, options: &WriteOptions) -> FsResult<Box<dyn FileWriter>> {
        self.fs().as_ref().open_writer(self.path(), options)
    }

    /// Reads the whole temporary file into memory.
    ///
    /// # Returns
    /// Complete temporary file bytes.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when opening or reading fails.
    fn read_all(&self) -> FsResult<Vec<u8>> {
        self.fs().as_ref().read_all(self.path())
    }

    /// Writes all bytes to the temporary file and commits the writer.
    ///
    /// # Parameters
    /// - `bytes`: Bytes to write.
    ///
    /// # Returns
    /// Write outcome reported by the owning filesystem.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when opening, writing, aborting, or committing
    /// fails.
    fn write_all(&self, bytes: &[u8]) -> FsResult<WriteOutcome> {
        self.fs().as_ref().write_all(self.path(), bytes)
    }

    /// Persists the temporary file to a target path.
    ///
    /// # Parameters
    /// - `target`: Final target path.
    /// - `options`: Persistence options.
    ///
    /// # Returns
    /// Write outcome for the persisted resource.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when persistence fails.
    fn persist(
        self: Box<Self>,
        target: &FsPath,
        options: &PersistOptions,
    ) -> FsResult<WriteOutcome>;
}
