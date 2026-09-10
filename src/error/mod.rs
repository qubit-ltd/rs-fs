// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Error types used by filesystem abstractions.

mod fs_effect_state;
mod fs_error;
mod fs_error_kind;
mod fs_operation;
mod fs_result;

pub use fs_effect_state::FsEffectState;
pub use fs_error::FsError;
pub use fs_error_kind::FsErrorKind;
pub use fs_operation::FsOperation;
pub use fs_result::FsResult;

mod open_failure;
pub use open_failure::OpenFailure;

mod open_failure_stage;
pub use open_failure_stage::OpenFailureStage;

mod recovery_cleanup_state;
pub use recovery_cleanup_state::RecoveryCleanupState;
