/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Error types used by filesystem abstractions.

mod fs_error;
mod fs_error_kind;
mod fs_operation;

pub use fs_error::FsError;
pub use fs_error::FsResult;
pub use fs_error_kind::FsErrorKind;
pub use fs_operation::FsOperation;
