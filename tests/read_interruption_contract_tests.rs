// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Synchronous aggregate reads retry only interruptions known to be safe.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsEffectState;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::read::PrefixReadTermination;
use qubit_fs::read::ReadOptions;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_io::Input;

const REQUEST_PATH: &str = "/payload";
const PROVIDER_ID: &str = "read-interrupt";

#[derive(Clone, Copy)]
enum TypedEffect {
    None,
    Unchanged,
    State(FsEffectState),
}

#[derive(Clone)]
enum Step {
    Data(&'static [u8]),
    OrdinaryInterrupted,
    TypedInterrupted(TypedEffect),
    OrdinaryError,
    Eof,
}

struct ScriptReader {
    steps: VecDeque<Step>,
    data: Option<(&'static [u8], usize)>,
    calls: Arc<AtomicUsize>,
}

impl Input for ScriptReader {
    type Item = u8;

    unsafe fn read_unchecked(&mut self, output: &mut [u8], index: usize, count: usize) -> io::Result<usize> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let target = &mut output[index..index + count];
        loop {
            if let Some((bytes, position)) = self.data {
                let length = target.len().min(bytes.len() - position);
                target[..length].copy_from_slice(&bytes[position..position + length]);
                self.data = (position + length < bytes.len()).then_some((bytes, position + length));
                return Ok(length);
            }
            match self.steps.pop_front().expect("script must end with an explicit EOF") {
                Step::Data(bytes) => self.data = Some((bytes, 0)),
                Step::OrdinaryInterrupted => {
                    return Err(io::Error::from(io::ErrorKind::Interrupted));
                }
                Step::TypedInterrupted(effect) => {
                    let mut error = FsError::with_source(
                        FsErrorKind::Interrupted,
                        FsOperation::Read,
                        "scripted typed interruption",
                        SourceMarker,
                    );
                    if let TypedEffect::State(state) = effect {
                        error = error.with_effect_state(state);
                    }
                    return Err(error.into_io_error());
                }
                Step::OrdinaryError => {
                    return Err(io::Error::other("scripted ordinary read error"));
                }
                Step::Eof => return Ok(0),
            }
        }
    }
}

#[derive(Debug)]
struct SourceMarker;

impl fmt::Display for SourceMarker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("typed read source marker")
    }
}

impl Error for SourceMarker {}

struct ReadSpi {
    steps: VecDeque<Step>,
    calls: Arc<AtomicUsize>,
}

impl FileSystemSpi for ReadSpi {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new(PROVIDER_ID).expect("valid id"),
                PROVIDER_ID,
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new().with(ProviderOperation::OpenReader),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("valid provider properties")
    }

    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Stat,
            "unexpected stat",
        ))
    }

    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Ok(OpenedReader::new(
            OpenedFileInfo::new(
                FileSystemId::new(PROVIDER_ID).expect("valid id"),
                request.path().clone(),
            ),
            Box::new(ScriptReader {
                steps: self.steps.clone(),
                data: None,
                calls: Arc::clone(&self.calls),
            }),
        ))
    }
}

fn filesystem(steps: impl IntoIterator<Item = Step>) -> (FileSystem, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let filesystem = FileSystem::from_spi(ReadSpi {
        steps: steps.into_iter().collect(),
        calls: Arc::clone(&calls),
    })
    .expect("filesystem");
    (filesystem, calls)
}

fn assert_safe_read(script: Vec<Step>, expected: &[u8], expected_calls: usize) {
    let (filesystem, calls) = filesystem(script);
    let path = Path::parse(REQUEST_PATH).expect("path");
    let bytes = filesystem
        .read_all(&path, ReadOptions::default(), 128)
        .expect("safe interruption should retry");
    assert_eq!(expected, bytes);
    assert_eq!(expected_calls, calls.load(Ordering::SeqCst));
}

#[test]
fn test_read_all_retries_safe_interruption_without_repeating_bytes() {
    assert_safe_read(vec![Step::OrdinaryInterrupted, Step::Data(b"ab"), Step::Eof], b"ab", 3);
    assert_safe_read(
        vec![
            Step::Data(b"ab"),
            Step::OrdinaryInterrupted,
            Step::Data(b"cd"),
            Step::Eof,
        ],
        b"abcd",
        4,
    );
    assert_safe_read(
        vec![
            Step::Data(b"ab"),
            Step::TypedInterrupted(TypedEffect::None),
            Step::Data(b"cd"),
            Step::Eof,
        ],
        b"abcd",
        4,
    );
    assert_safe_read(
        vec![
            Step::Data(b"ab"),
            Step::TypedInterrupted(TypedEffect::Unchanged),
            Step::Data(b"cd"),
            Step::Eof,
        ],
        b"abcd",
        4,
    );
}

fn assert_unsafe_typed_stops(effect: FsEffectState) {
    let (filesystem, calls) = filesystem([
        Step::TypedInterrupted(TypedEffect::State(effect)),
        Step::Data(b"must not be read"),
        Step::Eof,
    ]);
    let path = Path::parse(REQUEST_PATH).expect("path");
    let error = filesystem
        .read_all(&path, ReadOptions::default(), 128)
        .expect_err("unsafe effect must stop immediately");
    assert_eq!(FsErrorKind::Interrupted, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(effect), error.effect_state());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some(PROVIDER_ID), error.provider());
    let mut source = Error::source(&error);
    let mut has_marker = false;
    while let Some(current) = source {
        has_marker |= current.is::<SourceMarker>();
        source = current.source();
    }
    assert!(has_marker, "typed source marker must survive the stream boundary");
    assert_eq!(1, calls.load(Ordering::SeqCst));
}

#[test]
fn test_read_all_stops_on_typed_interruption_with_uncertain_or_applied_effect() {
    for effect in [
        FsEffectState::PartiallyApplied,
        FsEffectState::Applied,
        FsEffectState::Indeterminate,
    ] {
        assert_unsafe_typed_stops(effect);
    }
}

#[test]
fn test_read_all_returns_non_interruption_error_without_retry() {
    let (filesystem, calls) = filesystem([Step::OrdinaryError, Step::Eof]);
    let path = Path::parse(REQUEST_PATH).expect("path");
    let error = filesystem
        .read_all(&path, ReadOptions::default(), 128)
        .expect_err("ordinary error must be returned");
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some(PROVIDER_ID), error.provider());
    assert_eq!(1, calls.load(Ordering::SeqCst));
}

fn assert_safe_prefix(script: Vec<Step>, expected: &[u8], expected_calls: usize) {
    let (filesystem, calls) = filesystem(script);
    let path = Path::parse(REQUEST_PATH).expect("path");
    let outcome = filesystem
        .read_prefix(&path, ReadOptions::default(), 64)
        .expect("safe interruption should retry");
    assert_eq!(expected, outcome.bytes());
    assert_eq!(PrefixReadTermination::StreamEnded, outcome.termination());
    assert_eq!(expected_calls, calls.load(Ordering::SeqCst));
}

#[test]
fn test_read_prefix_retries_safe_interruption_without_repeating_bytes() {
    assert_safe_prefix(vec![Step::OrdinaryInterrupted, Step::Data(b"ab"), Step::Eof], b"ab", 3);
    assert_safe_prefix(
        vec![
            Step::Data(b"ab"),
            Step::OrdinaryInterrupted,
            Step::Data(b"cd"),
            Step::Eof,
        ],
        b"abcd",
        4,
    );
    assert_safe_prefix(
        vec![
            Step::Data(b"ab"),
            Step::TypedInterrupted(TypedEffect::None),
            Step::Data(b"cd"),
            Step::Eof,
        ],
        b"abcd",
        4,
    );
    assert_safe_prefix(
        vec![
            Step::Data(b"ab"),
            Step::TypedInterrupted(TypedEffect::Unchanged),
            Step::Data(b"cd"),
            Step::Eof,
        ],
        b"abcd",
        4,
    );
}

#[test]
fn test_read_prefix_stops_on_typed_interruption_with_uncertain_or_applied_effect() {
    for effect in [
        FsEffectState::PartiallyApplied,
        FsEffectState::Applied,
        FsEffectState::Indeterminate,
    ] {
        let (filesystem, calls) = filesystem([
            Step::TypedInterrupted(TypedEffect::State(effect)),
            Step::Data(b"must not be read"),
            Step::Eof,
        ]);
        let path = Path::parse(REQUEST_PATH).expect("path");
        let error = filesystem
            .read_prefix(&path, ReadOptions::default(), 64)
            .expect_err("unsafe effect must stop immediately");
        assert_eq!(FsErrorKind::Interrupted, error.kind());
        assert_eq!(FsOperation::Read, error.operation());
        assert_eq!(Some(effect), error.effect_state());
        assert_eq!(Some(&path), error.path());
        assert_eq!(Some(PROVIDER_ID), error.provider());
        let mut source = Error::source(&error);
        let mut has_marker = false;
        while let Some(current) = source {
            has_marker |= current.is::<SourceMarker>();
            source = current.source();
        }
        assert!(has_marker, "typed source marker must survive the stream boundary");
        assert_eq!(1, calls.load(Ordering::SeqCst));
    }
}

#[test]
fn test_read_prefix_returns_non_interruption_error_without_retry() {
    let (filesystem, calls) = filesystem([Step::OrdinaryError, Step::Eof]);
    let path = Path::parse(REQUEST_PATH).expect("path");
    let error = filesystem
        .read_prefix(&path, ReadOptions::default(), 64)
        .expect_err("ordinary error must be returned");
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some(PROVIDER_ID), error.provider());
    assert_eq!(1, calls.load(Ordering::SeqCst));
}

#[test]
fn test_read_prefix_limit_ends_without_extra_stream_probe() {
    let (filesystem, calls) = filesystem([Step::Data(b"abc"), Step::Eof]);
    let path = Path::parse(REQUEST_PATH).expect("path");
    let outcome = filesystem
        .read_prefix(&path, ReadOptions::default(), 3)
        .expect("prefix");
    assert_eq!(b"abc", outcome.bytes());
    assert_eq!(PrefixReadTermination::LimitReached, outcome.termination());
    assert_eq!(1, calls.load(Ordering::SeqCst));
}
