// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Typed rename failure state shared across the facade and provider boundary.

mod rename_failure;
mod rename_failure_state;
mod rename_outcome_validation;
mod rename_outcome_validation_error;

pub use rename_failure::RenameFailure;
pub use rename_failure_state::RenameFailureState;
pub(crate) use rename_outcome_validation::validate_rename_outcome;
