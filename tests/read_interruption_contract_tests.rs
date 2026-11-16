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
const CUSTOM_PROVIDER_ID: &str = "read-custom";

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
    TypedInterruptedKindMismatch,
    TypedInterruptedFromCustomProvider,
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
                    error = match effect {
                        TypedEffect::None => error,
                        TypedEffect::Unchanged => error.with_effect_state(FsEffectState::Unchanged),
                        TypedEffect::State(state) => error.with_effect_state(state),
                    };
                    error = error
                        .with_path(Path::parse(REQUEST_PATH).expect("script path"))
                        .with_provider(PROVIDER_ID);
                    return Err(error.into_io_error());
                }
                Step::TypedInterruptedKindMismatch => {
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        FsError::with_source(
                            FsErrorKind::Io,
                            FsOperation::Read,
                            "scripted conflicting interruption",
                            SourceMarker,
                        )
                        .with_effect_state(FsEffectState::Applied)
                        .with_path(Path::parse(REQUEST_PATH).expect("script path"))
                        .with_provider(PROVIDER_ID),
                    ));
                }
                Step::TypedInterruptedFromCustomProvider => {
                    return Err(FsError::with_source(
                        FsErrorKind::Interrupted,
                        FsOperation::Read,
                        "scripted typed interruption",
                        SourceMarker,
                    )
                    .with_provider(CUSTOM_PROVIDER_ID)
                    .with_path(Path::parse(REQUEST_PATH).expect("script path"))
                    .with_effect_state(FsEffectState::Applied)
                    .into_io_error());
                }
                Step::OrdinaryError => {
                    return Err(io::Error::other("scripted ordinary read error"));
                }
                Step::Eof => return Ok(0),
            }
        }
    }
}

#[test]
fn test_script_reader_preserves_typed_effect_evidence() {
    for (typed_effect, expected_effect) in [
        (TypedEffect::None, None),
        (TypedEffect::Unchanged, Some(FsEffectState::Unchanged)),
        (TypedEffect::State(FsEffectState::Applied), Some(FsEffectState::Applied)),
    ] {
        let mut reader = ScriptReader {
            steps: [Step::TypedInterrupted(typed_effect)].into_iter().collect(),
            data: None,
            calls: Arc::new(AtomicUsize::new(0)),
        };
        let mut output = [0; 1];
        let io_error = unsafe { reader.read_unchecked(&mut output, 0, 1) }.expect_err("typed interruption");
        assert_eq!(io::ErrorKind::Interrupted, io_error.kind());
        let fs_error = io_error
            .get_ref()
            .and_then(|source| source.downcast_ref::<FsError>())
            .expect("typed FsError");
        assert_eq!(expected_effect, fs_error.effect_state());
        assert!(Error::source(fs_error).is_some_and(|source| source.is::<SourceMarker>()));
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
fn test_read_all_preserves_typed_error_provider() {
    let (filesystem, calls) = filesystem([Step::TypedInterruptedFromCustomProvider]);
    let path = Path::parse(REQUEST_PATH).expect("path");
    let error = filesystem
        .read_all(&path, ReadOptions::default(), 128)
        .expect_err("typed error must be returned");
    assert_eq!(FsErrorKind::Interrupted, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some(CUSTOM_PROVIDER_ID), error.provider());
    assert_eq!(Some(FsEffectState::Applied), error.effect_state());
    assert!(Error::source(&error).is_some_and(|source| source.is::<SourceMarker>()));
    assert_eq!(1, calls.load(Ordering::SeqCst));
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
    assert_eq!(None, error.effect_state());
    assert!(Error::source(&error).is_some_and(|source| {
        source
            .downcast_ref::<io::Error>()
            .is_some_and(|source| source.kind() == io::ErrorKind::Other)
    }));
    assert_eq!(1, calls.load(Ordering::SeqCst));
}

#[test]
fn test_direct_read_fully_stops_on_unsafe_interruption() {
    let (fs, calls) = filesystem([
        Step::TypedInterrupted(TypedEffect::State(FsEffectState::Applied)),
        Step::Data(b"after"),
        Step::Eof,
    ]);
    let path = Path::parse(REQUEST_PATH).expect("path");
    let mut reader = fs.open_reader(&path, ReadOptions::default()).expect("reader");
    let mut output = [0; 5];
    let error = reader
        .read_fully(&mut output)
        .expect_err("unsafe interruption must stop");
    assert_eq!(io::ErrorKind::Other, error.kind());
    let typed = error
        .get_ref()
        .and_then(|e| e.downcast_ref::<FsError>())
        .expect("FsError payload");
    assert_eq!(FsErrorKind::Interrupted, typed.kind());
    assert_eq!(FsOperation::Read, typed.operation());
    assert_eq!(Some(FsEffectState::Applied), typed.effect_state());
    assert_eq!(Some(&path), typed.path());
    assert_eq!(Some(PROVIDER_ID), typed.provider());
    assert_eq!(1, calls.load(Ordering::SeqCst));
}

#[test]
fn test_direct_aggregate_reads_stop_on_each_unsafe_typed_effect() {
    for effect in [
        FsEffectState::Applied,
        FsEffectState::PartiallyApplied,
        FsEffectState::Indeterminate,
    ] {
        for use_exactly in [false, true] {
            let (fs, calls) = filesystem([
                Step::TypedInterrupted(TypedEffect::State(effect)),
                Step::Data(b"after"),
                Step::Eof,
            ]);
            let path = Path::parse(REQUEST_PATH).expect("path");
            let mut reader = fs.open_reader(&path, ReadOptions::default()).expect("reader");
            let mut output = [0; 5];
            let error = if use_exactly {
                reader.read_exactly(&mut output)
            } else {
                reader.read_fully(&mut output).map(|_| ())
            }
            .expect_err("unsafe typed interruption must stop");
            assert_eq!(io::ErrorKind::Other, error.kind());
            let typed = error
                .get_ref()
                .and_then(|e| e.downcast_ref::<FsError>())
                .expect("direct FsError payload");
            assert_eq!(FsErrorKind::Interrupted, typed.kind());
            assert_eq!(FsOperation::Read, typed.operation());
            assert_eq!(Some(effect), typed.effect_state());
            assert_eq!(Some(&path), typed.path());
            assert_eq!(Some(PROVIDER_ID), typed.provider());
            assert!(Error::source(typed).is_some_and(|source| source.is::<SourceMarker>()));
            assert_eq!(1, calls.load(Ordering::SeqCst));
        }
    }
}

#[test]
fn test_direct_aggregate_reads_retry_safe_interruption() {
    for effect in [TypedEffect::None, TypedEffect::Unchanged] {
        for use_exactly in [false, true] {
            let (fs, calls) = filesystem([Step::TypedInterrupted(effect), Step::Data(b"after"), Step::Eof]);
            let path = Path::parse(REQUEST_PATH).expect("path");
            let mut reader = fs.open_reader(&path, ReadOptions::default()).expect("reader");
            let mut output = [0; 5];
            if use_exactly {
                reader.read_exactly(&mut output).expect("safe interruption retries");
            } else {
                assert_eq!(5, reader.read_fully(&mut output).expect("safe interruption retries"));
            }
            assert_eq!(b"after", &output);
            assert_eq!(2, calls.load(Ordering::SeqCst));
        }
    }
    for use_exactly in [false, true] {
        let (fs, calls) = filesystem([Step::OrdinaryInterrupted, Step::Data(b"after"), Step::Eof]);
        let path = Path::parse(REQUEST_PATH).expect("path");
        let mut reader = fs.open_reader(&path, ReadOptions::default()).expect("reader");
        let mut output = [0; 5];
        if use_exactly {
            reader.read_exactly(&mut output).expect("ordinary interruption retries");
        } else {
            assert_eq!(
                5,
                reader.read_fully(&mut output).expect("ordinary interruption retries")
            );
        }
        assert_eq!(b"after", &output);
        assert_eq!(2, calls.load(Ordering::SeqCst));
    }
}

#[test]
fn test_direct_read_preserves_unsafe_payload_and_partial_buffer() {
    let (fs, calls) = filesystem([
        Step::Data(b"ab"),
        Step::TypedInterrupted(TypedEffect::State(FsEffectState::Applied)),
        Step::Data(b"cde"),
        Step::Eof,
    ]);
    let path = Path::parse(REQUEST_PATH).expect("path");
    let mut reader = fs.open_reader(&path, ReadOptions::default()).expect("reader");
    let mut output = [0; 5];
    let error = reader.read_fully(&mut output).expect_err("unsafe interruption stops");
    assert_eq!(io::ErrorKind::Other, error.kind());
    let typed = error
        .get_ref()
        .and_then(|e| e.downcast_ref::<FsError>())
        .expect("FsError payload");
    assert_eq!(FsErrorKind::Interrupted, typed.kind());
    assert_eq!(Some(FsEffectState::Applied), typed.effect_state());
    assert_eq!(b"ab", &output[..2]);
    assert_eq!(2, calls.load(Ordering::SeqCst));

    let (fs, calls) = filesystem([
        Step::TypedInterrupted(TypedEffect::State(FsEffectState::Applied)),
        Step::Data(b"after"),
        Step::Eof,
    ]);
    let mut reader = fs.open_reader(&path, ReadOptions::default()).expect("reader");
    let error = reader.read(&mut output).expect_err("unsafe interruption");
    assert_eq!(io::ErrorKind::Other, error.kind());
    let typed = error
        .get_ref()
        .and_then(|e| e.downcast_ref::<FsError>())
        .expect("direct FsError payload");
    assert_eq!(FsErrorKind::Interrupted, typed.kind());
    assert_eq!(Some(FsEffectState::Applied), typed.effect_state());
    assert_eq!(FsOperation::Read, typed.operation());
    assert_eq!(Some(&path), typed.path());
    assert_eq!(Some(PROVIDER_ID), typed.provider());
    assert!(Error::source(typed).is_some_and(|source| source.is::<SourceMarker>()));
    assert_eq!(1, calls.load(Ordering::SeqCst));
}

#[test]
fn test_direct_aggregate_reads_return_ordinary_errors_without_retry() {
    for use_exactly in [false, true] {
        let (fs, calls) = filesystem([Step::OrdinaryError, Step::Data(b"after"), Step::Eof]);
        let path = Path::parse(REQUEST_PATH).expect("path");
        let mut reader = fs.open_reader(&path, ReadOptions::default()).expect("reader");
        let mut output = [0; 5];
        let error = if use_exactly {
            reader.read_exactly(&mut output)
        } else {
            reader.read_fully(&mut output).map(|_| ())
        }
        .expect_err("ordinary errors must be returned");
        assert_eq!(io::ErrorKind::Other, error.kind());
        assert_eq!("scripted ordinary read error", error.to_string());
        assert_eq!(1, calls.load(Ordering::SeqCst));
    }
}

#[test]
fn test_direct_read_reclassifies_conflicting_typed_error_but_preserves_other_errors() {
    let (fs, calls) = filesystem([Step::TypedInterruptedKindMismatch, Step::Data(b"after"), Step::Eof]);
    let path = Path::parse(REQUEST_PATH).expect("path");
    let mut reader = fs.open_reader(&path, ReadOptions::default()).expect("reader");
    let mut output = [0; 5];
    let error = reader.read(&mut output).expect_err("conflicting typed error stops");
    assert_eq!(io::ErrorKind::Other, error.kind());
    let typed = error
        .get_ref()
        .and_then(|e| e.downcast_ref::<FsError>())
        .expect("FsError payload");
    assert_eq!(FsErrorKind::Io, typed.kind());
    assert_eq!(FsOperation::Read, typed.operation());
    assert_eq!(Some(FsEffectState::Applied), typed.effect_state());
    assert_eq!(Some(&path), typed.path());
    assert_eq!(Some(PROVIDER_ID), typed.provider());
    assert!(Error::source(typed).is_some_and(|source| source.is::<SourceMarker>()));
    assert_eq!(1, calls.load(Ordering::SeqCst));

    let (fs, calls) = filesystem([Step::OrdinaryError, Step::Eof]);
    let mut reader = fs.open_reader(&path, ReadOptions::default()).expect("reader");
    let error = reader.read(&mut output).expect_err("ordinary error");
    assert_eq!(io::ErrorKind::Other, error.kind());
    assert_eq!("scripted ordinary read error", error.to_string());
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
fn test_read_prefix_preserves_typed_error_provider() {
    let (filesystem, calls) = filesystem([Step::TypedInterruptedFromCustomProvider]);
    let path = Path::parse(REQUEST_PATH).expect("path");
    let error = filesystem
        .read_prefix(&path, ReadOptions::default(), 64)
        .expect_err("typed error must be returned");
    assert_eq!(FsErrorKind::Interrupted, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some(CUSTOM_PROVIDER_ID), error.provider());
    assert_eq!(Some(FsEffectState::Applied), error.effect_state());
    assert!(Error::source(&error).is_some_and(|source| source.is::<SourceMarker>()));
    assert_eq!(1, calls.load(Ordering::SeqCst));
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
    assert_eq!(None, error.effect_state());
    assert!(Error::source(&error).is_some_and(|source| {
        source
            .downcast_ref::<io::Error>()
            .is_some_and(|source| source.kind() == io::ErrorKind::Other)
    }));
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
