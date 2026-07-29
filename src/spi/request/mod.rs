// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Unforgeable requests passed from the facade to providers.
//!
//! ```compile_fail
//! use qubit_fs::{Path, spi::StatRequest};
//!
//! let path = Path::root();
//! let _ = StatRequest::new(&path);
//! ```

mod requests;

pub use requests::*;
