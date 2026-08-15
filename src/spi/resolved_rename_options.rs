// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Facade-resolved rename options.

use super::internal::resolved_options;
use crate::rename::RenameOptions;

resolved_options!(ResolvedRenameOptions, RenameOptions);
