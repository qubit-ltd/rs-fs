// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-opened synchronous reader envelope.

use qubit_io::ByteInput;

use crate::metadata::OpenedFileInfo;

/// An already-open provider reader.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::{FileKind, FileMetadata, FileSystemId, OpenedFileInfo};
/// use qubit_fs::path::Path;
/// use qubit_fs::spi::OpenedReader;
/// use qubit_fs::FsError;
/// use std::io::Cursor;
///
/// fn open_reader() -> Result<OpenedReader, FsError> {
///     let info = OpenedFileInfo::new(FileSystemId::new("doc")?, Path::parse("/report")?)
///         .with_metadata(FileMetadata::new(FileKind::File));
///     Ok(OpenedReader::new(info, Box::new(Cursor::new(b"report".to_vec()))))
/// }
///
/// assert!(open_reader().is_ok());
/// ```
#[must_use = "transfer the opened provider reader or explicitly drop its session"]
pub struct OpenedReader {
    /// Resource identity claimed by the provider.
    info: OpenedFileInfo,
    /// Provider byte-input session.
    reader: Box<dyn ByteInput + Send>,
}

impl OpenedReader {
    /// Wraps a reader which the provider has fully opened.
    ///
    /// # Parameters
    /// - `info`: Identity and metadata claimed for the opened resource.
    /// - `reader`: Provider reader session.
    ///
    /// # Returns
    /// An opened-reader envelope for facade validation.
    #[inline]
    pub fn new(info: OpenedFileInfo, reader: Box<dyn ByteInput + Send>) -> Self {
        Self { info, reader }
    }

    /// Returns the opened reader to the facade.
    ///
    /// # Returns
    /// The claimed resource identity and provider reader session.
    #[inline]
    #[must_use = "transfer or drop the provider reader together with its metadata"]
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn ByteInput + Send>) {
        (self.info, self.reader)
    }
}
