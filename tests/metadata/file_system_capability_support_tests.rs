// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::metadata::FileSystemCapabilitySupport;

#[test]
fn capability_support_statuses_are_debuggable_and_distinct() {
    assert_ne!(
        FileSystemCapabilitySupport::Unsupported,
        FileSystemCapabilitySupport::Conditional,
    );
    assert_ne!(
        FileSystemCapabilitySupport::Conditional,
        FileSystemCapabilitySupport::Guaranteed,
    );
    assert_eq!("Guaranteed", format!("{:?}", FileSystemCapabilitySupport::Guaranteed),);
}
