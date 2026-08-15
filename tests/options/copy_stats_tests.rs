// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::copy::CopyStats;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;

#[test]
fn test_copy_stats_add_assign_adds_all_counters() {
    let mut stats = CopyStats {
        files: 1,
        directories: 2,
        symlinks: 3,
        objects: 4,
        prefixes: 5,
        bytes: 6,
        overwritten: 7,
        skipped: 8,
        failed: 9,
    };

    stats
        .add_assign(&CopyStats {
            files: 10,
            directories: 20,
            symlinks: 30,
            objects: 40,
            prefixes: 50,
            bytes: 60,
            overwritten: 70,
            skipped: 80,
            failed: 90,
        })
        .expect("ordinary statistics accumulation should succeed");

    assert_eq!(11, stats.files);
    assert_eq!(22, stats.directories);
    assert_eq!(33, stats.symlinks);
    assert_eq!(44, stats.objects);
    assert_eq!(55, stats.prefixes);
    assert_eq!(66, stats.bytes);
    assert_eq!(77, stats.overwritten);
    assert_eq!(88, stats.skipped);
    assert_eq!(99, stats.failed);
}

/// Verifies counter overflow is reported without partially mutating the
/// accumulated statistics.
#[test]
fn test_copy_stats_add_assign_rejects_overflow_atomically() {
    let mut stats = CopyStats {
        files: u64::MAX,
        ..CopyStats::default()
    };
    let error = stats
        .add_assign(&CopyStats {
            files: 1,
            ..CopyStats::default()
        })
        .expect_err("counter overflow must be rejected");

    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(FsOperation::Copy, error.operation());
    assert_eq!(u64::MAX, stats.files);
}
