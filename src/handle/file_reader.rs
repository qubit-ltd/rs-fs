// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete synchronous file reader handle.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};
use std::io::Result as IoResult;

use qubit_io::Input;

use crate::OpenedFileInfo;

/// Type-erased byte input explicitly associated with an opened file.
pub struct FileReader {
    inner: Box<dyn Input<Item = u8> + Send>,
    info: OpenedFileInfo,
}

impl FileReader {
    /// Wraps a provider byte input with its fixed file identity.
    ///
    /// Calling this constructor is the explicit provider adaptation step. An
    /// arbitrary [`Input`] does not automatically become a file reader.
    ///
    /// # Parameters
    /// - `inner`: Already-open byte input.
    /// - `info`: File identity and optional open-time metadata snapshot.
    ///
    /// # Returns
    /// A concrete file reader handle.
    #[inline]
    #[must_use]
    pub(crate) fn new(
        info: OpenedFileInfo,
        inner: Box<dyn Input<Item = u8> + Send>,
    ) -> Self {
        Self { inner, info }
    }

    /// Returns the fixed identity and open-time metadata snapshot.
    ///
    /// # Returns
    /// Information captured when the reader was opened.
    #[inline]
    #[must_use]
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }
}

impl Input for FileReader {
    type Item = u8;

    #[inline]
    fn is_buffered(&self) -> bool {
        self.inner.is_buffered()
    }

    #[inline]
    unsafe fn read_unchecked(
        &mut self,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> IoResult<usize> {
        // SAFETY: The caller guarantees the same range contract required by
        // the wrapped input.
        unsafe { self.inner.read_unchecked(output, index, count) }
    }
}

impl Debug for FileReader {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("FileReader")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}
