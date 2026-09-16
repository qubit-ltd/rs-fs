// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public synchronous and asynchronous prefix-read contract regression tests.

#[cfg(feature = "async")]
#[path = "common/poll_support.rs"]
mod poll_support;
#[path = "support/prefix_read.rs"]
mod prefix_read_support;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use prefix_read_support::Fault;
use prefix_read_support::Observations;
use prefix_read_support::RecordingProvider;
#[cfg(feature = "async")]
use qubit_fs::AsyncFileSystem;
use qubit_fs::FileSystem;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemLimit;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::ResourceVersion;
use qubit_fs::read::ChecksumPolicy;
use qubit_fs::read::PrefixReadTermination;
use qubit_fs::read::ReadOptions;

/// Exercises dispatch and byte accounting for both facade implementations.
fn assert_prefix(async_mode: bool) {
    let observations = Arc::new(Observations::default());
    let provider = RecordingProvider {
        capabilities: FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::Read)
            .with_guaranteed(FileSystemCapability::RangeRead),
        limits: FileSystemLimits::unknown(),
        observations: Arc::clone(&observations),
        fault: Fault::None,
    };
    let path = Path::parse("/payload").unwrap();
    let result = if async_mode {
        #[cfg(feature = "async")]
        {
            poll_support::ready(AsyncFileSystem::from_spi(provider).unwrap().read_prefix(
                &path,
                ReadOptions::default(),
                3,
            ))
        }
        #[cfg(not(feature = "async"))]
        {
            unreachable!("async feature required")
        }
    } else {
        FileSystem::from_spi(provider)
            .unwrap()
            .read_prefix(&path, ReadOptions::default(), 3)
    }
    .unwrap();
    assert_eq!(result.bytes(), b"abc");
    assert_eq!(observations.opens.load(Ordering::SeqCst), 1);
    assert_eq!(observations.stats.load(Ordering::SeqCst), 0);
    assert_eq!(observations.read_bytes.load(Ordering::SeqCst), 3);
    assert_eq!(observations.seen.lock().unwrap()[0].length(), Some(3));
}

/// Guaranteed range reads receive a bounded provider request.
#[test]
fn guaranteed_range_bounds_sync_request() {
    assert_prefix(false);
}

#[test]
fn test_prefix_outcome_limit_preserves_request_facts() {
    let observations = Arc::new(Observations::default());
    let provider = RecordingProvider {
        capabilities: ranged(),
        limits: FileSystemLimits::unknown(),
        observations,
        fault: Fault::None,
    };
    let filesystem = FileSystem::from_spi(provider).unwrap();
    let path = Path::parse("/payload").unwrap();
    let options = ReadOptions::default();
    let outcome = filesystem.read_prefix(&path, options.clone(), 3).unwrap();
    assert_eq!(outcome.bytes(), b"abc");
    assert_eq!(outcome.info().path(), &path);
    assert_eq!(outcome.options(), &options);
    assert_eq!(outcome.max_bytes(), 3);
    assert_eq!(outcome.termination(), PrefixReadTermination::LimitReached);
    assert_eq!(outcome.into_bytes(), b"abc");
}

#[test]
fn test_prefix_outcome_eof_and_zero_limit() {
    let observations = Arc::new(Observations::default());
    let provider = RecordingProvider {
        capabilities: sequential(),
        limits: FileSystemLimits::unknown(),
        observations: Arc::clone(&observations),
        fault: Fault::None,
    };
    let filesystem = FileSystem::from_spi(provider).unwrap();
    let path = Path::parse("/payload").unwrap();
    let outcome = filesystem.read_prefix(&path, ReadOptions::default(), 32).unwrap();
    assert_eq!(outcome.termination(), PrefixReadTermination::StreamEnded);
    let zero = filesystem.read_prefix(&path, ReadOptions::default(), 0).unwrap();
    assert!(zero.bytes().is_empty());
    assert_eq!(zero.termination(), PrefixReadTermination::LimitReached);
    assert_eq!(observations.opens.load(Ordering::SeqCst), 2);
}

#[cfg(feature = "async")]
#[test]
fn test_prefix_outcome_async_exposes_termination() {
    let observations = Arc::new(Observations::default());
    let provider = RecordingProvider {
        capabilities: sequential(),
        limits: FileSystemLimits::unknown(),
        observations,
        fault: Fault::None,
    };
    let path = Path::parse("/payload").unwrap();
    let outcome = poll_support::ready(AsyncFileSystem::from_spi(provider).unwrap().read_prefix(
        &path,
        ReadOptions::default(),
        32,
    ))
    .unwrap();
    assert_eq!(outcome.bytes(), b"abcdefghijklmnop");
    assert_eq!(outcome.termination(), PrefixReadTermination::StreamEnded);
}

/// Asynchronous dispatch follows the same bounded request contract.
#[cfg(feature = "async")]
#[test]
fn guaranteed_range_bounds_async_request() {
    assert_prefix(true);
}

/// Runs an arbitrary request in each enabled execution mode with fresh state.
fn check_case(
    capabilities: FileSystemCapabilities,
    limits: FileSystemLimits,
    options: ReadOptions,
    maximum: usize,
    expected: Result<(ReadOptions, &[u8]), FsErrorKind>,
) {
    let modes = if cfg!(feature = "async") {
        &[false, true][..]
    } else {
        &[false][..]
    };
    for &async_mode in modes {
        let observations = Arc::new(Observations::default());
        let provider = RecordingProvider {
            capabilities,
            limits,
            observations: Arc::clone(&observations),
            fault: Fault::None,
        };
        let path = Path::parse("/payload").unwrap();
        let result = if async_mode {
            #[cfg(feature = "async")]
            {
                poll_support::ready(AsyncFileSystem::from_spi(provider).unwrap().read_prefix(
                    &path,
                    options.clone(),
                    maximum,
                ))
            }
            #[cfg(not(feature = "async"))]
            {
                unreachable!("async feature required")
            }
        } else {
            FileSystem::from_spi(provider)
                .unwrap()
                .read_prefix(&path, options.clone(), maximum)
        };
        assert_eq!(observations.stats.load(Ordering::SeqCst), 0);
        match &expected {
            Ok((seen, bytes)) => {
                assert_eq!(result.unwrap().bytes(), *bytes);
                assert_eq!(observations.opens.load(Ordering::SeqCst), 1);
                assert_eq!(observations.seen.lock().unwrap().as_slice(), std::slice::from_ref(seen));
                assert_eq!(observations.read_bytes.load(Ordering::SeqCst), bytes.len());
            }
            Err(kind) => {
                let error = result.unwrap_err();
                assert_eq!(error.kind(), *kind);
                assert_eq!(error.path(), Some(&path));
                assert_eq!(observations.opens.load(Ordering::SeqCst), 0);
                assert_eq!(observations.read_bytes.load(Ordering::SeqCst), 0);
            }
        }
    }
}

/// Returns the basic sequential read guarantee.
fn sequential() -> FileSystemCapabilities {
    FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read)
}

/// Returns the stronger guaranteed byte-range contract.
fn ranged() -> FileSystemCapabilities {
    sequential().with_guaranteed(FileSystemCapability::RangeRead)
}

/// Explicit windows are narrowed without moving the offset or expanding length.
#[test]
fn explicit_range_is_only_narrowed() {
    let options = ReadOptions::default().with_offset(Some(2)).with_length(Some(10));
    check_case(
        ranged(),
        FileSystemLimits::unknown(),
        options.clone(),
        3,
        Ok((options.with_length(Some(3)), b"cde")),
    );
    let options = ReadOptions::default().with_length(Some(2));
    check_case(
        ranged(),
        FileSystemLimits::unknown(),
        options.clone(),
        3,
        Ok((options, b"ab")),
    );
}

/// Optional support is insufficient to insert a new range requirement.
#[test]
fn conditional_and_unsupported_ranges_keep_sequential_reads() {
    for capabilities in [
        sequential(),
        sequential().with_conditional(FileSystemCapability::RangeRead),
    ] {
        check_case(
            capabilities,
            FileSystemLimits::unknown(),
            ReadOptions::default(),
            3,
            Ok((ReadOptions::default(), b"abc")),
        );
    }
    check_case(
        sequential(),
        FileSystemLimits::unknown(),
        ReadOptions::default().with_length(Some(2)),
        3,
        Err(FsErrorKind::RequirementNotMet),
    );
}

/// Narrowing never hides a caller's out-of-budget range.
#[test]
fn validates_original_range_before_narrowing() {
    let limits = FileSystemLimits::unknown().with_max_read_range_bytes(FileSystemLimit::Maximum(8));
    check_case(
        ranged(),
        limits,
        ReadOptions::default().with_length(Some(100)),
        3,
        Err(FsErrorKind::ResourceLimitExceeded),
    );
    check_case(
        ranged(),
        limits,
        ReadOptions::default(),
        20,
        Ok((ReadOptions::default(), b"abcdefghijklmnop")),
    );
}

/// Prefix operations never claim full checksum completion.
#[test]
fn checksum_policy_prevents_unsafe_range_insertion() {
    let capabilities = ranged().with_guaranteed(FileSystemCapability::ChecksumValidation);
    let options = ReadOptions::default().with_checksum(ChecksumPolicy::BestEffort);
    check_case(
        capabilities,
        FileSystemLimits::unknown(),
        options.clone(),
        3,
        Ok((options, b"abc")),
    );
    for maximum in [0, 3, 100] {
        check_case(
            capabilities,
            FileSystemLimits::unknown(),
            ReadOptions::default().with_checksum(ChecksumPolicy::Required),
            maximum,
            Err(FsErrorKind::RequirementNotMet),
        );
    }
}

/// Zero prefixes preserve the single open without requesting or consuming
/// bytes.
#[test]
fn zero_prefix_preserves_original_open_options() {
    check_case(
        ranged(),
        FileSystemLimits::unknown(),
        ReadOptions::default(),
        0,
        Ok((ReadOptions::default(), b"")),
    );
}

/// Offset arithmetic overflow leaves the original request to the provider.
#[test]
fn overflowing_candidate_is_not_inserted() {
    let options = ReadOptions::default().with_offset(Some(u64::MAX));
    check_case(
        ranged(),
        FileSystemLimits::unknown(),
        options.clone(),
        3,
        Ok((options, b"")),
    );
}

/// Contradictory conditions are rejected before provider I/O.
#[test]
fn invalid_conditions_are_not_hidden_by_prefix_planning() {
    let options = ReadOptions::default()
        .with_if_match(Some(ResourceVersion::new("a")))
        .with_if_none_match(Some(ResourceVersion::new("b")));
    check_case(
        ranged(),
        FileSystemLimits::unknown(),
        options,
        3,
        Err(FsErrorKind::InvalidOptions),
    );
}

/// The aggregate operation still requires the basic read capability.
#[test]
fn absent_read_capability_rejects_before_open() {
    check_case(
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        ReadOptions::default(),
        3,
        Err(FsErrorKind::UnsupportedCapability),
    );
}

/// Open and stream failures retain their typed cause without a retry.
#[test]
fn provider_failures_preserve_sources_and_do_not_retry() {
    use std::error::Error;
    let modes = if cfg!(feature = "async") {
        &[false, true][..]
    } else {
        &[false][..]
    };
    for &asynchronous in modes {
        for fault in [Fault::Open, Fault::Stream] {
            let observations = Arc::new(Observations::default());
            let provider = RecordingProvider {
                capabilities: ranged(),
                limits: FileSystemLimits::unknown(),
                observations: Arc::clone(&observations),
                fault,
            };
            let path = Path::parse("/payload").unwrap();
            let result = if asynchronous {
                #[cfg(feature = "async")]
                {
                    poll_support::ready(AsyncFileSystem::from_spi(provider).unwrap().read_prefix(
                        &path,
                        ReadOptions::default(),
                        3,
                    ))
                }
                #[cfg(not(feature = "async"))]
                {
                    unreachable!("async feature required")
                }
            } else {
                FileSystem::from_spi(provider)
                    .unwrap()
                    .read_prefix(&path, ReadOptions::default(), 3)
            };
            let error = result.unwrap_err();
            assert_eq!(error.kind(), FsErrorKind::PermissionDenied);
            assert_eq!(error.path(), Some(&path));
            assert!(error.source().is_some());
            let mut source: &dyn Error = &error;
            while let Some(next) = source.source() {
                source = next;
            }
            assert_eq!(source.to_string(), "prefix provider source");
            assert_eq!(observations.opens.load(Ordering::SeqCst), 1);
            assert_eq!(observations.stats.load(Ordering::SeqCst), 0);
        }
    }
}
