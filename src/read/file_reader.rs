// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete synchronous file reader handle.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::io::Result as IoResult;

use qubit_io::ByteInput;
use qubit_io::Input;

use crate::error::FsEffectState;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::metadata::OpenedFileInfo;

/// Type-erased byte input explicitly associated with an opened file.
///
/// Aggregate reads retry ordinary interruptions and typed interruptions whose
/// effect is absent or [`FsEffectState::Unchanged`]. A typed interruption with
/// another effect, or with a conflicting error kind, is returned with outer
/// [`IoErrorKind::Other`]; its original [`FsError`] remains the direct error
/// payload. A failed aggregate read may already have written bytes into its
/// destination buffer; those bytes are not rolled back.
///
/// # Examples
///
/// The example runs against an isolated in-memory fixture. Applications obtain
/// their configured facade from a provider or registry integration.
///
/// ```rust
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # let filesystem = support::rustdoc_provider::filesystem();
/// use qubit_fs::Path;
/// use qubit_fs::read::ReadOptions;
/// use qubit_io::Input;
///
/// let mut reader = filesystem.open_reader(&Path::parse("/report")?, ReadOptions::default())?;
/// let mut prefix = [0; 3];
/// assert_eq!(3, reader.read_fully(&mut prefix)?);
/// assert_eq!(*b"rep", prefix);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct FileReader {
    /// Provider byte input.
    inner: Box<dyn ByteInput + Send>,
    /// Stable identity and metadata captured at open time.
    info: OpenedFileInfo,
}

impl FileReader {
    /// Wraps a provider byte input with its fixed file identity.
    ///
    /// Calling this constructor is the explicit provider adaptation step. An
    /// arbitrary [`Input`] does not automatically become a file reader.
    ///
    /// # Parameters
    /// - `info`: File identity and optional open-time metadata snapshot.
    /// - `inner`: Already-open byte input.
    ///
    /// # Returns
    /// A concrete file reader handle.
    #[inline]
    #[must_use]
    pub(crate) fn new(info: OpenedFileInfo, inner: Box<dyn ByteInput + Send>) -> Self {
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
    /// Byte value yielded by this synchronous reader.
    type Item = u8;

    /// Reports whether the wrapped byte input can satisfy reads from its
    /// buffer.
    #[inline]
    fn is_buffered(&self) -> bool {
        self.inner.is_buffered()
    }

    /// Reads the requested byte range under the caller's `Input` safety
    /// contract.
    ///
    /// # Parameters
    /// - `output`: Caller-owned destination buffer.
    /// - `index`: Start index of the requested output range.
    /// - `count`: Number of bytes requested from that range.
    ///
    /// # Safety
    /// `index..index + count` must be a valid range within `output`, and the
    /// addition must not overflow.
    #[inline]
    unsafe fn read_unchecked(&mut self, output: &mut [u8], index: usize, count: usize) -> IoResult<usize> {
        // SAFETY: The caller guarantees the same range contract required by
        // the wrapped input.
        unsafe { self.inner.read_unchecked(output, index, count) }.map_err(|error| {
            if error.kind() != IoErrorKind::Interrupted {
                return error;
            }
            let unsafe_typed = error
                .get_ref()
                .and_then(|source| source.downcast_ref::<FsError>())
                .is_some_and(|typed| {
                    typed.kind() != FsErrorKind::Interrupted
                        || !matches!(typed.effect_state(), None | Some(FsEffectState::Unchanged))
                });
            if unsafe_typed {
                let source = error.into_inner().expect("typed interruption retains its source");
                IoError::other(source)
            } else {
                error
            }
        })
    }
}

impl Debug for FileReader {
    /// Formats the reader's stable file information without exposing its input.
    ///
    /// # Parameters
    /// - `formatter`: Destination for the debug representation.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("FileReader")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}
