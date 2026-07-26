// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::io::Result as IoResult;
use std::sync::{Arc, Mutex};

use qubit_fs::{
    AchievedAtomicity, FileLocation, FileSystemId, FileWriteSession, FileWriter, FsError,
    FsErrorKind, FsOperation, FsPath, OpenedFileInfo, PublicationMethod, WriteFailure,
    WriteFailureState, WriteOutcome, WriterState,
};
use qubit_io::Output;

#[derive(Debug)]
struct TestWriteSession {
    bytes: Arc<Mutex<Vec<u8>>>,
    commit_failure_once: Option<WriteFailureState>,
    abort_error: Option<FsErrorKind>,
    buffered: bool,
    aborts: Arc<Mutex<usize>>,
}

impl Output for TestWriteSession {
    type Item = u8;

    fn is_buffered(&self) -> bool {
        self.buffered
    }

    unsafe fn write_unchecked(
        &mut self,
        input: &[u8],
        index: usize,
        count: usize,
    ) -> IoResult<usize> {
        self.bytes
            .lock()
            .expect("bytes lock should succeed")
            .extend_from_slice(&input[index..index + count]);
        Ok(count)
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl FileWriteSession for TestWriteSession {
    fn commit(&mut self) -> Result<WriteOutcome, WriteFailure> {
        if let Some(state) = self.commit_failure_once.take() {
            let kind = if state == WriteFailureState::Indeterminate {
                FsErrorKind::Indeterminate
            } else {
                FsErrorKind::Io
            };
            return Err(WriteFailure::new(
                FsError::new(kind, FsOperation::CommitWriter, "commit failure"),
                state,
            ));
        }
        Ok(WriteOutcome::new(
            AchievedAtomicity::Atomic,
            PublicationMethod::Direct,
        ))
    }

    fn abort(&mut self) -> qubit_fs::FsResult<()> {
        *self.aborts.lock().expect("abort lock should succeed") += 1;
        if let Some(kind) = self.abort_error {
            return Err(FsError::new(kind, FsOperation::AbortWriter, "abort failed"));
        }
        Ok(())
    }
}

fn opened_info() -> OpenedFileInfo {
    OpenedFileInfo::new(FileLocation::new(
        FileSystemId::new("mock-instance").expect("id should parse"),
        FsPath::parse_normalized("/out.bin").expect("path should parse"),
    ))
}

#[test]
fn commit_failure_retains_the_writer_session_for_retry() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let aborts = Arc::new(Mutex::new(0));
    let mut writer = FileWriter::new(
        TestWriteSession {
            bytes: bytes.clone(),
            commit_failure_once: Some(WriteFailureState::Retryable),
            abort_error: None,
            buffered: false,
            aborts,
        },
        opened_info(),
    );

    writer
        .write_fully(b"payload")
        .expect("write should succeed");
    assert!(writer.commit().is_err());
    assert_eq!(WriterState::Open, writer.state());
    assert!(writer.commit().is_ok());
    assert_eq!(WriterState::Committed, writer.state());
    assert_eq!(
        b"payload",
        bytes.lock().expect("lock should succeed").as_slice()
    );
}

#[test]
fn not_published_commit_failure_retains_session_for_abort() {
    let aborts = Arc::new(Mutex::new(0));
    let mut writer = FileWriter::new(
        TestWriteSession {
            bytes: Arc::new(Mutex::new(Vec::new())),
            commit_failure_once: Some(WriteFailureState::NotPublished),
            abort_error: None,
            buffered: false,
            aborts: aborts.clone(),
        },
        opened_info(),
    );

    assert_eq!(FsErrorKind::Io, writer.commit().unwrap_err().kind());
    assert_eq!(WriterState::NotPublished, writer.state());
    assert_eq!(
        FsErrorKind::InvalidState,
        writer.commit().unwrap_err().kind(),
    );
    writer
        .abort()
        .expect("terminal session should be abortable");
    assert_eq!(WriterState::Aborted, writer.state());
    assert_eq!(1, *aborts.lock().expect("lock should succeed"));
}

#[test]
fn published_commit_failure_retains_session_for_cleanup() {
    let aborts = Arc::new(Mutex::new(0));
    let mut writer = FileWriter::new(
        TestWriteSession {
            bytes: Arc::new(Mutex::new(Vec::new())),
            commit_failure_once: Some(WriteFailureState::Published),
            abort_error: None,
            buffered: false,
            aborts: aborts.clone(),
        },
        opened_info(),
    );

    assert_eq!(FsErrorKind::Io, writer.commit().unwrap_err().kind());
    assert_eq!(WriterState::Published, writer.state());
    writer
        .abort()
        .expect("published session cleanup should be allowed");
    assert_eq!(WriterState::Aborted, writer.state());
    assert_eq!(1, *aborts.lock().expect("lock should succeed"));
}

#[test]
fn writer_rejects_io_after_commit_and_repeated_lifecycle_calls() {
    let mut writer = FileWriter::new(
        TestWriteSession {
            bytes: Arc::new(Mutex::new(Vec::new())),
            commit_failure_once: None,
            abort_error: None,
            buffered: false,
            aborts: Arc::new(Mutex::new(0)),
        },
        opened_info(),
    );

    writer.commit().expect("commit should succeed");
    assert!(format!("{writer:?}").contains("Committed"));
    let write_error = writer
        .write(b"late")
        .expect_err("closed writer should reject I/O");
    assert_eq!(std::io::ErrorKind::BrokenPipe, write_error.kind());
    assert_eq!(
        FsOperation::Write,
        write_error
            .get_ref()
            .and_then(|error| error.downcast_ref::<FsError>())
            .expect("stream error should retain FsError")
            .operation(),
    );
    let commit_error = writer.commit().expect_err("second commit should fail");
    assert_eq!(FsErrorKind::InvalidState, commit_error.kind());
    assert_eq!(FsOperation::CommitWriter, commit_error.operation());
    let abort_error = writer.abort().expect_err("abort after commit should fail");
    assert_eq!(FsErrorKind::InvalidState, abort_error.kind());
    assert_eq!(FsOperation::AbortWriter, abort_error.operation());
    assert_eq!(
        std::io::ErrorKind::BrokenPipe,
        writer
            .flush()
            .expect_err("closed writer should reject flush")
            .kind(),
    );
}

#[test]
fn explicit_abort_transitions_state_and_prevents_drop_abort() {
    let aborts = Arc::new(Mutex::new(0));
    {
        let mut writer = FileWriter::new(
            TestWriteSession {
                bytes: Arc::new(Mutex::new(Vec::new())),
                commit_failure_once: None,
                abort_error: None,
                buffered: false,
                aborts: aborts.clone(),
            },
            opened_info(),
        );
        writer.abort().expect("abort should succeed");
        assert_eq!(WriterState::Aborted, writer.state());
    }
    assert_eq!(1, *aborts.lock().expect("lock should succeed"));
}

#[test]
fn indeterminate_commit_retains_session_for_explicit_abort() {
    let aborts = Arc::new(Mutex::new(0));
    let mut writer = FileWriter::new(
        TestWriteSession {
            bytes: Arc::new(Mutex::new(Vec::new())),
            commit_failure_once: Some(WriteFailureState::Indeterminate),
            abort_error: None,
            buffered: true,
            aborts: aborts.clone(),
        },
        opened_info(),
    );

    assert!(writer.is_buffered());
    writer.flush().expect("open writer should flush");
    assert_eq!("/out.bin", writer.info().location().path().as_str());
    assert_eq!(
        FsErrorKind::Indeterminate,
        writer.commit().unwrap_err().kind(),
    );
    assert_eq!(WriterState::Indeterminate, writer.state());
    assert_eq!(
        FsErrorKind::InvalidState,
        writer.commit().unwrap_err().kind(),
    );
    writer
        .abort()
        .expect("indeterminate writer should be abortable");
    assert_eq!(WriterState::Aborted, writer.state());
    assert_eq!(1, *aborts.lock().expect("lock should succeed"));
}

#[test]
fn abort_failure_retains_open_session_and_drop_retries_cleanup() {
    let aborts = Arc::new(Mutex::new(0));
    {
        let mut writer = FileWriter::new(
            TestWriteSession {
                bytes: Arc::new(Mutex::new(Vec::new())),
                commit_failure_once: None,
                abort_error: Some(FsErrorKind::Io),
                buffered: false,
                aborts: aborts.clone(),
            },
            opened_info(),
        );

        assert!(writer.abort().is_err());
        assert_eq!(WriterState::Open, writer.state());
    }
    assert_eq!(2, *aborts.lock().expect("lock should succeed"));
}

#[test]
fn dropping_an_open_writer_performs_best_effort_abort() {
    let aborts = Arc::new(Mutex::new(0));
    {
        let _writer = FileWriter::new(
            TestWriteSession {
                bytes: Arc::new(Mutex::new(Vec::new())),
                commit_failure_once: None,
                abort_error: None,
                buffered: false,
                aborts: aborts.clone(),
            },
            opened_info(),
        );
    }
    assert_eq!(1, *aborts.lock().expect("lock should succeed"));
}

#[test]
fn indeterminate_abort_disables_automatic_drop_abort() {
    let aborts = Arc::new(Mutex::new(0));
    {
        let mut writer = FileWriter::new(
            TestWriteSession {
                bytes: Arc::new(Mutex::new(Vec::new())),
                commit_failure_once: None,
                abort_error: Some(FsErrorKind::Indeterminate),
                buffered: false,
                aborts: aborts.clone(),
            },
            opened_info(),
        );

        assert_eq!(
            FsErrorKind::Indeterminate,
            writer.abort().unwrap_err().kind(),
        );
        assert_eq!(WriterState::Indeterminate, writer.state());
    }
    assert_eq!(1, *aborts.lock().expect("lock should succeed"));
}

#[test]
fn indeterminate_commit_is_not_automatically_aborted_on_drop() {
    let aborts = Arc::new(Mutex::new(0));
    {
        let mut writer = FileWriter::new(
            TestWriteSession {
                bytes: Arc::new(Mutex::new(Vec::new())),
                commit_failure_once: Some(WriteFailureState::Indeterminate),
                abort_error: None,
                buffered: false,
                aborts: aborts.clone(),
            },
            opened_info(),
        );

        assert_eq!(
            FsErrorKind::Indeterminate,
            writer.commit().unwrap_err().kind(),
        );
        assert_eq!(WriterState::Indeterminate, writer.state());
    }
    assert_eq!(0, *aborts.lock().expect("lock should succeed"));
}
