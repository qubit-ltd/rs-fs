// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade tests.
//! Concrete asynchronous file reader handle.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::io::Result as IoResult;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use qubit_io::AsyncByteInput;
use qubit_io::AsyncInput;
use qubit_io::BoxAsyncInput;

use crate::metadata::OpenedFileInfo;

/// Type-erased asynchronous byte input associated with an opened file.
///
/// Asynchronous providers must report temporary unavailability as
/// `Poll::Pending` and arrange to wake the registered task. Returning
/// `WouldBlock` or `Interrupted` violates the async I/O contract. The safe
/// `qubit-io` read methods expose these errors as `InvalidData` while retaining
/// the original cause chain. Consequently, `read_all` and `read_prefix`
/// classify them as `FsErrorKind::Io`; they do not promote a nested `FsError`
/// to the outer error or promise that a nested error's type/effect is
/// recoverable through `Error::source`. A directly transported typed business
/// error keeps its type, kind, provider, and effect state, but a failed read
/// may already have changed the caller's buffer, so its kind does not prove
/// that the buffer is unchanged. Synchronous interruption retry rules do not
/// apply to asynchronous inputs.
///
/// # Examples
///
/// The application supplies a configured filesystem and a validated path.
///
/// ```rust
/// use qubit_fs::AsyncFileSystem;
/// use qubit_fs::Path;
/// use qubit_fs::read::ReadOptions;
/// use qubit_io::AsyncInput;
///
/// async fn read_prefix(
///     filesystem: &AsyncFileSystem,
///     path: &Path,
/// ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
///     let mut reader = filesystem.open_reader(path, ReadOptions::default()).await?;
///     let mut prefix = vec![0; 16];
///     let count = reader.read_fully_async(&mut prefix).await?;
///     prefix.truncate(count);
///     Ok(prefix)
/// }
/// ```
pub struct AsyncFileReader {
    /// Pinned provider byte input.
    inner: BoxAsyncInput<dyn AsyncByteInput + Send>,
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
    pub(crate) fn new(info: OpenedFileInfo, inner: Box<dyn AsyncByteInput + Send>) -> Self {
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
    /// Byte value yielded by this asynchronous reader.
    type Item = u8;

    /// Reports whether the provider input can satisfy reads from buffered data.
    ///
    /// # Returns
    /// `true` when the wrapped input already buffers bytes for subsequent
    /// reads.
    #[inline]
    fn is_buffered(&self) -> bool {
        self.inner.is_buffered()
    }

    /// Polls the provider input using the caller's unsafe range contract.
    ///
    /// # Parameters
    /// - `cx`: Task context used to register readiness when the read is
    ///   pending.
    /// - `output`: Destination storage receiving the read bytes.
    /// - `index`: Start of the destination range.
    /// - `count`: Maximum number of bytes requested.
    ///
    /// # Returns
    /// The wrapped input's pending state or its ready byte count/error.
    ///
    /// # Safety
    /// `index..index + count` must be a valid range within `output`, and the
    /// addition must not overflow.
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
    /// Formats the open-time file information without exposing the input.
    ///
    /// # Parameters
    /// - `formatter`: Destination for the debug representation.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncFileReader")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}
