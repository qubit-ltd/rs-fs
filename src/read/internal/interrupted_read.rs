// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================

//! Retry policy for recoverable synchronous stream interruptions.

use std::io;

use qubit_io::Input;

use crate::error::FsEffectState;
use crate::error::FsError;
use crate::error::FsErrorKind;

fn retryable(error: &io::Error) -> bool {
    if let Some(fs_error) = error.get_ref().and_then(|source| source.downcast_ref::<FsError>()) {
        return fs_error.kind() == FsErrorKind::Interrupted
            && matches!(fs_error.effect_state(), None | Some(FsEffectState::Unchanged));
    }
    error.kind() == io::ErrorKind::Interrupted
}

/// Reads once or retries a recoverable synchronous interruption.
pub(crate) fn read_retry_interrupted<I: Input<Item = u8> + ?Sized>(
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
