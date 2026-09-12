// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-opened synchronous writer envelope.

use super::FileWriterSpi;
use crate::metadata::OpenedFileInfo;

/// An already-open provider writer.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::error::FsResult;
/// use qubit_fs::metadata::{AchievedAtomicity, FileKind, FileMetadata, FileSystemId, OpenedFileInfo, PublicationMethod, WriteOutcome};
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::{FileWriterSpi, SpiWriteFailure};
/// use qubit_fs::spi::OpenedWriter;
/// use qubit_fs::write::WriteAbortOutcome;
/// use qubit_io::Output;
/// use std::io::Result as IoResult;
///
/// struct Writer;
/// impl Output for Writer {
///     type Item = u8;
///     unsafe fn write_unchecked(&mut self, _: &[u8], _: usize, count: usize) -> IoResult<usize> {
///         Ok(count)
///     }
///     fn flush(&mut self) -> IoResult<()> {
///         Ok(())
///     }
/// }
/// impl FileWriterSpi for Writer {
///     fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
///         Ok(WriteOutcome::new(AchievedAtomicity::NonAtomic, PublicationMethod::Direct))
///     }
///     fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
///         Ok(WriteAbortOutcome::NotPublished)
///     }
/// }
/// let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/draft")?)
///     .with_metadata(FileMetadata::new(FileKind::File));
/// let _writer = OpenedWriter::new(info, Box::new(Writer));
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
pub struct OpenedWriter {
    /// Resource identity claimed by the provider.
    info: OpenedFileInfo,
    /// Provider write session.
    writer: Box<dyn FileWriterSpi>,
}

impl OpenedWriter {
    /// Wraps a writer which the provider has fully opened.
    ///
    /// # Parameters
    /// - `info`: Identity and metadata claimed for the opened resource.
    /// - `writer`: Provider writer session.
    ///
    /// # Returns
    /// An opened-writer envelope for facade validation.
    #[inline]
    #[must_use]
    pub fn new(info: OpenedFileInfo, writer: Box<dyn FileWriterSpi>) -> Self {
        Self { info, writer }
    }

    /// Returns the opened writer to the facade.
    ///
    /// # Returns
    /// The claimed resource identity and provider writer session.
    #[inline]
    #[must_use]
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn FileWriterSpi>) {
        (self.info, self.writer)
    }
}
