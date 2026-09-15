// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade.
//! Facade-resolved directory-creation options.

use super::internal::resolved_options;
use crate::directory::CreateDirectoryOptions;

resolved_options!(ResolvedCreateDirectoryOptions, CreateDirectoryOptions);
