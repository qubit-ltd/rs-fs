// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Facade-resolved deletion options.

use super::internal::resolved_options;
use crate::directory::DeleteOptions;

resolved_options!(ResolvedDeleteOptions, DeleteOptions);
