// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;
use std::io::Error as IoError;
use std::io::Result as IoResult;
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
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::FileWriterSpi;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiWriteFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::write::WriteAbortOutcome;
use qubit_fs::write::WriteFailureState;
use qubit_fs::write::WriteOptions;
use qubit_io::Output;

#[cfg(feature = "async")]
#[path = "common/poll_support.rs"]
mod poll_support;

const PAYLOAD: &[u8] = b"payload";

#[derive(Debug)]
struct MarkerCause;

impl std::fmt::Display for MarkerCause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("marker cause")
    }
}

impl Error for MarkerCause {}

#[derive(Clone, Copy)]
enum FailureStage {
    Write,
    Flush,
}

#[derive(Clone, Copy)]
enum ErrorContext {
    TypedProvider,
    TypedMissingProvider,
    Untyped,
}

struct Counts {
    commits: AtomicUsize,
}

struct Provider {
    stage: FailureStage,
    context: ErrorContext,
    counts: Arc<Counts>,
}

struct Session {
    stage: FailureStage,
    context: ErrorContext,
    counts: Arc<Counts>,
    accepted: usize,
}

fn stream_error(context: ErrorContext) -> IoError {
    let error = match context {
        ErrorContext::Untyped => return IoError::other(MarkerCause),
        ErrorContext::TypedProvider | ErrorContext::TypedMissingProvider => {
            let error = FsError::with_source(
                FsErrorKind::Io,
                FsOperation::Write,
                "scripted stream failure",
                MarkerCause,
            )
            .with_effect_state(FsEffectState::Indeterminate);
            match context {
                ErrorContext::TypedProvider => error.with_provider("inner-provider"),
                ErrorContext::TypedMissingProvider => error,
                ErrorContext::Untyped => unreachable!(),
            }
        }
    };
    error.into_io_error()
}

impl Output for Session {
    type Item = u8;

    unsafe fn write_unchecked(&mut self, _: &[u8], _: usize, count: usize) -> IoResult<usize> {
        if matches!(self.stage, FailureStage::Write) {
            Err(stream_error(self.context))
        } else {
            self.accepted += count;
            Ok(count)
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        if matches!(self.stage, FailureStage::Flush) {
            Err(stream_error(self.context))
        } else {
            Ok(())
        }
    }
}

impl FileWriterSpi for Session {
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
        self.counts.commits.fetch_add(1, Ordering::SeqCst);
        panic!("commit must not run after a stream failure")
    }

    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        Ok(WriteAbortOutcome::NotPublished)
    }
}

impl FileSystemSpi for Provider {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("provider-context").unwrap(),
                "outer-provider",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new().with(ProviderOperation::OpenWriter),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Write),
            FileSystemLimits::unknown(),
            qubit_fs::path::PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }

    fn stat(&self, _request: StatRequest<'_>) -> FsResult<StatResponse> {
        Err(FsError::new(
            FsErrorKind::NotFound,
            FsOperation::Stat,
            "unused",
        ))
    }

    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        let info = OpenedFileInfo::new(
            FileSystemId::new("provider-context").unwrap(),
            request.path().clone(),
        )
        .with_metadata(FileMetadata::new(FileKind::File));
        Ok(OpenedWriter::new(
            info,
            Box::new(Session {
                stage: self.stage,
                context: self.context,
                counts: Arc::clone(&self.counts),
                accepted: 0,
            }),
        ))
    }
}

fn run_sync(stage: FailureStage, context: ErrorContext) -> bool {
    let counts = Arc::new(Counts {
        commits: AtomicUsize::new(0),
    });
    let filesystem = FileSystem::from_spi(Provider {
        stage,
        context,
        counts: Arc::clone(&counts),
    })
    .expect("facade should construct");
    let path = Path::parse("/target").expect("test path should parse");
    let mut failure = filesystem
        .write_all(&path, PAYLOAD, WriteOptions::default())
        .expect_err("scripted stream error should fail write_all");
    let expected_provider = match context {
        ErrorContext::TypedProvider => "inner-provider",
        ErrorContext::TypedMissingProvider | ErrorContext::Untyped => "outer-provider",
    };
    let error = failure.error();
    let provider_matches = Some(expected_provider) == error.provider();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::Write, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(
        if matches!(context, ErrorContext::Untyped) {
            None
        } else {
            Some(FsEffectState::Indeterminate)
        },
        error.effect_state(),
    );
    let source = Error::source(error).expect("stream cause should be retained");
    if let Some(cause) = source.downcast_ref::<MarkerCause>() {
        let _ = cause;
    } else if let Some(io) = source.downcast_ref::<IoError>() {
        assert!(
            io.get_ref()
                .is_some_and(|source| source.is::<MarkerCause>())
        );
    } else {
        panic!("unexpected source type");
    }
    let expected_bytes = if matches!(stage, FailureStage::Write) {
        0
    } else {
        PAYLOAD.len() as u64
    };
    assert_eq!(expected_bytes, failure.written_bytes());
    assert_eq!(WriteFailureState::Indeterminate, failure.state());
    assert!(
        failure.take_recovery().is_some(),
        "opened recovery must be retained"
    );
    assert_eq!(0, counts.commits.load(Ordering::SeqCst));
    provider_matches
}

#[test]
fn sync_write_all_preserves_or_fills_provider_context() {
    let mut mismatches = Vec::new();
    for stage in [FailureStage::Write, FailureStage::Flush] {
        for context in [
            ErrorContext::TypedProvider,
            ErrorContext::TypedMissingProvider,
            ErrorContext::Untyped,
        ] {
            if !run_sync(stage, context) {
                mismatches.push(match stage {
                    FailureStage::Write => "write",
                    FailureStage::Flush => "flush",
                });
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "provider mismatches at stages: {mismatches:?}"
    );
}

#[cfg(feature = "async")]
mod asynchronous {
    use super::*;
    use qubit_fs::AsyncFileSystem;
    use qubit_fs::spi::AsyncFileSystemSpi;
    use qubit_fs::spi::AsyncFileWriteSession;
    use qubit_fs::spi::OpenedAsyncWriter;
    use qubit_fs::spi::SpiFuture;
    use qubit_fs::write::WriteFailure;
    use qubit_io::AsyncOutput;
    use std::pin::Pin;
    use std::task::Context;
    use std::task::Poll;

    struct AsyncSession(Session);

    impl AsyncOutput for AsyncSession {
        type Item = u8;

        unsafe fn poll_write_unchecked(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &[u8],
            _: usize,
            count: usize,
        ) -> Poll<IoResult<usize>> {
            let this = self.get_mut();
            if matches!(this.0.stage, FailureStage::Write) {
                Poll::Ready(Err(stream_error(this.0.context)))
            } else {
                this.0.accepted += count;
                Poll::Ready(Ok(count))
            }
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
            let this = self.get_mut();
            Poll::Ready(if matches!(this.0.stage, FailureStage::Flush) {
                Err(stream_error(this.0.context))
            } else {
                Ok(())
            })
        }
    }

    impl AsyncFileWriteSession for AsyncSession {
        fn commit_async<'a>(
            self: Pin<&'a mut Self>,
        ) -> SpiFuture<'a, Result<WriteOutcome, WriteFailure>> {
            self.get_mut()
                .0
                .counts
                .commits
                .fetch_add(1, Ordering::SeqCst);
            Box::pin(async { panic!("commit must not run after a stream failure") })
        }

        fn abort_async(self: Pin<&mut Self>) -> SpiFuture<'_, FsResult<WriteAbortOutcome>> {
            Box::pin(async { Ok(WriteAbortOutcome::NotPublished) })
        }
    }

    impl AsyncFileSystemSpi for Provider {
        fn properties(&self) -> ProviderProperties {
            <Provider as FileSystemSpi>::properties(self)
        }

        fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            Box::pin(async move { <Provider as FileSystemSpi>::stat(self, request) })
        }

        fn open_writer<'a>(
            &'a self,
            request: OpenWriterRequest<'a>,
        ) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
            let info = OpenedFileInfo::new(
                FileSystemId::new("provider-context").unwrap(),
                request.path().clone(),
            )
            .with_metadata(FileMetadata::new(FileKind::File));
            let session = AsyncSession(Session {
                stage: self.stage,
                context: self.context,
                counts: Arc::clone(&self.counts),
                accepted: 0,
            });
            Box::pin(async move { Ok(OpenedAsyncWriter::new(info, Box::new(session))) })
        }
    }

    async fn run(stage: FailureStage, context: ErrorContext) -> bool {
        let counts = Arc::new(Counts {
            commits: AtomicUsize::new(0),
        });
        let filesystem = AsyncFileSystem::from_spi(Provider {
            stage,
            context,
            counts: Arc::clone(&counts),
        })
        .expect("facade should construct");
        let path = Path::parse("/target").expect("test path should parse");
        let mut operation = filesystem
            .begin_write_all(path.clone(), PAYLOAD.to_vec(), WriteOptions::default())
            .expect("operation should construct");
        let failure = operation
            .execute()
            .await
            .expect_err("scripted error should fail");
        let expected_provider = match context {
            ErrorContext::TypedProvider => "inner-provider",
            ErrorContext::TypedMissingProvider | ErrorContext::Untyped => "outer-provider",
        };
        let provider_matches = Some(expected_provider) == failure.error().provider();
        assert_eq!(FsErrorKind::Io, failure.error().kind());
        assert_eq!(FsOperation::Write, failure.error().operation());
        assert_eq!(Some(&path), failure.error().path());
        assert_eq!(
            if matches!(context, ErrorContext::Untyped) {
                None
            } else {
                Some(FsEffectState::Indeterminate)
            },
            failure.error().effect_state(),
        );
        let source = Error::source(failure.error()).expect("stream cause should be retained");
        assert!(
            source.downcast_ref::<MarkerCause>().is_some()
                || source.downcast_ref::<IoError>().is_some_and(|io| {
                    io.get_ref()
                        .is_some_and(|source| source.is::<MarkerCause>())
                })
        );
        assert_eq!(
            if matches!(stage, FailureStage::Write) {
                0
            } else {
                PAYLOAD.len() as u64
            },
            failure.written_bytes()
        );
        assert_eq!(WriteFailureState::Indeterminate, failure.state());
        assert_eq!(
            qubit_fs::write::AsyncWriteAllOperationState::Failed(WriteFailureState::Indeterminate),
            operation.state()
        );
        assert!(operation.has_recovery());
        assert_eq!(0, counts.commits.load(Ordering::SeqCst));
        provider_matches
    }

    #[test]
    fn async_write_all_preserves_or_fills_provider_context() {
        poll_support::ready(async {
            let mut mismatches = Vec::new();
            for stage in [FailureStage::Write, FailureStage::Flush] {
                for context in [
                    ErrorContext::TypedProvider,
                    ErrorContext::TypedMissingProvider,
                    ErrorContext::Untyped,
                ] {
                    if !run(stage, context).await {
                        mismatches.push(match stage {
                            FailureStage::Write => "write",
                            FailureStage::Flush => "flush",
                        });
                    }
                }
            }
            assert!(
                mismatches.is_empty(),
                "provider mismatches at stages: {mismatches:?}"
            );
        });
    }
}
