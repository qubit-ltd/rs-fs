// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Retry policy for recoverable synchronous stream interruptions.

use std::io;

use qubit_io::ByteInput;
use qubit_io::Input;

use crate::error::FsEffectState;
use crate::error::FsError;
use crate::error::FsErrorKind;

/// Reports whether an interruption is safe to retry without losing effects.
///
/// Filesystem errors are retryable only when their effect state is absent or
/// explicitly unchanged; ordinary interrupted I/O errors are retryable.
fn retryable(error: &io::Error) -> bool {
    if let Some(fs_error) = error.get_ref().and_then(|source| source.downcast_ref::<FsError>()) {
        return fs_error.kind() == FsErrorKind::Interrupted
            && matches!(fs_error.effect_state(), None | Some(FsEffectState::Unchanged));
    }
    error.kind() == io::ErrorKind::Interrupted
}

/// Reads from `input`, retrying interruptions known not to have changed state.
///
/// `before_read` runs before each read attempt, including retries. An ordinary
/// I/O interruption is retried; a filesystem interruption is retried only when
/// its effect state is absent or explicitly unchanged. Other errors, including
/// filesystem interruptions with uncertain or applied effects, are returned
/// immediately.
///
/// # Parameters
///
/// * `input` is the byte input to read.
/// * `output` receives bytes read by the input.
/// * `before_read` performs per-attempt setup and can stop the operation by
///   returning an error.
///
/// # Returns
///
/// The number of bytes read, or zero when the input reaches end of stream.
///
/// # Errors
///
/// Returns an error from `before_read`, a non-retryable input error, or a
/// retryable input error after any later setup attempt fails.
pub(crate) fn read_retry_interrupted<I: ByteInput + ?Sized>(
    input: &mut I,
    output: &mut [u8],
    mut before_read: impl FnMut() -> io::Result<()>,
) -> io::Result<usize> {
    loop {
        before_read()?;
        match Input::read(input, output) {
            Err(error) if retryable(&error) => {}
            result => return result,
        }
    }
}
