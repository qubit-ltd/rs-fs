// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Concrete asynchronous file reader handle.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};
use std::io::Result as IoResult;
use std::pin::Pin;
use std::task::{
    Context,
    Poll,
};

use qubit_io::{
    AsyncInput,
    BoxAsyncInput,
};

use crate::OpenedFileInfo;

/// Type-erased asynchronous byte input associated with an opened file.
pub struct AsyncFileReader {
    /// Pinned provider byte input.
    inner: BoxAsyncInput<dyn AsyncInput<Item = u8> + Send>,
    /// Stable identity and metadata captured at open time.
    info: OpenedFileInfo,
}

impl AsyncFileReader {
    /// Wraps an already-open asynchronous provider byte input.
    ///
    /// # Parameters
    /// - `inner`: Runtime-neutral asynchronous byte input.
    /// - `info`: File identity and optional open-time metadata snapshot.
    ///
    /// # Returns
    /// A pinned, type-erased asynchronous file reader.
    #[inline]
    #[must_use]
    pub(crate) fn new(
        info: OpenedFileInfo,
        inner: Box<dyn AsyncInput<Item = u8> + Send>,
    ) -> Self {
        Self {
            inner: BoxAsyncInput::new(inner),
            info,
        }
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

impl AsyncInput for AsyncFileReader {
    type Item = u8;

    #[inline]
    fn is_buffered(&self) -> bool {
        self.inner.is_buffered()
    }

    #[inline]
    unsafe fn poll_read_unchecked(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        let this = self.get_mut();
        // SAFETY: The caller guarantees the same range contract required by
        // the wrapped asynchronous input.
        unsafe {
            Pin::new(&mut this.inner)
                .get_pin_mut()
                .poll_read_unchecked(cx, output, index, count)
        }
    }
}

impl Debug for AsyncFileReader {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncFileReader")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}
