//! Regression coverage for publication facts reported by repeated commits.

use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::write::FileWriter;
use qubit_fs::write::WriteFailureState;
use qubit_fs::write::WriteOptions;
use qubit_fs::write::WriterState;

#[path = "handle_support/mod.rs"]
mod handle_support;

fn opened_writer(commit_failure: Option<WriteFailureState>) -> FileWriter {
    handle_support::writer_lifecycle_filesystem(commit_failure, None)
        .open_writer(
            &Path::parse("/target").expect("path should parse"),
            WriteOptions::default(),
        )
        .expect("writer should open")
}

#[test]
fn repeated_commit_preserves_known_sync_publication_state() {
    let (filesystem, commit_calls, abort_calls) = handle_support::writer_lifecycle_filesystem_with_counts(None, None);
    let mut writer = filesystem
        .open_writer(
            &Path::parse("/target").expect("path should parse"),
            WriteOptions::default(),
        )
        .expect("writer should open");
    writer.commit().expect("first commit should succeed");

    let failure = writer
        .commit()
        .expect_err("committed writer must reject a second commit");
    assert_eq!(FsErrorKind::InvalidState, failure.error().kind());
    assert_eq!(WriteFailureState::Published, failure.state());
    assert_eq!(WriterState::Committed, writer.state());
    assert_eq!(1, *commit_calls.lock().expect("counter lock should succeed"));
    assert_eq!(0, *abort_calls.lock().expect("counter lock should succeed"));
}

#[test]
fn repeated_commit_preserves_failed_sync_publication_states() {
    for (provider_state, expected_writer_state) in [
        (WriteFailureState::NotPublished, WriterState::NotPublished),
        (WriteFailureState::Published, WriterState::Published),
        (WriteFailureState::Indeterminate, WriterState::Indeterminate),
    ] {
        let mut writer = opened_writer(Some(provider_state));
        let first = writer.commit().expect_err("configured commit should fail");
        assert_eq!(provider_state, first.state());
        assert_eq!(expected_writer_state, writer.state());

        let repeated = writer.commit().expect_err("non-open writer must reject commit");
        assert_eq!(FsErrorKind::InvalidState, repeated.error().kind());
        assert_eq!(provider_state, repeated.state());
        assert_eq!(expected_writer_state, writer.state());
    }
}

#[test]
fn retryable_sync_commit_remains_retryable() {
    let (filesystem, commit_calls, abort_calls) =
        handle_support::writer_lifecycle_filesystem_with_counts(Some(WriteFailureState::RetryableNotPublished), None);
    let mut writer = filesystem
        .open_writer(
            &Path::parse("/target").expect("path should parse"),
            WriteOptions::default(),
        )
        .expect("writer should open");
    let first = writer.commit().expect_err("configured commit should fail");
    assert_eq!(WriteFailureState::RetryableNotPublished, first.state());
    assert_eq!(WriterState::Open, writer.state());

    // A retry must reach the provider again. The fixture deliberately keeps
    // returning the same failure; an InvalidState result would prove that the
    // retryable state had accidentally become terminal.
    let retry = writer.commit().expect_err("retry should reach the provider");
    assert_eq!(WriteFailureState::RetryableNotPublished, retry.state());
    assert_eq!(WriterState::Open, writer.state());
    assert_eq!(2, *commit_calls.lock().expect("counter lock should succeed"));

    let _ = writer.abort().expect("abort should succeed");
    assert_eq!(1, *abort_calls.lock().expect("counter lock should succeed"));
    let _ = writer.commit().expect_err("aborted writer must reject commit");
    assert_eq!(2, *commit_calls.lock().expect("counter lock should succeed"));
    assert_eq!(1, *abort_calls.lock().expect("counter lock should succeed"));
}

#[test]
fn repeated_commit_after_sync_abort_reports_not_published() {
    let mut writer = opened_writer(Some(WriteFailureState::NotPublished));
    writer.commit().expect_err("configured commit should fail");
    let _ = writer.abort().expect("staging cleanup should succeed");
    assert_eq!(WriterState::Aborted, writer.state());

    let failure = writer.commit().expect_err("aborted writer must reject commit");
    assert_eq!(FsErrorKind::InvalidState, failure.error().kind());
    assert_eq!(WriteFailureState::NotPublished, failure.state());
    assert_eq!(WriterState::Aborted, writer.state());
}

#[cfg(feature = "async")]
#[path = "common/async_recording_spi.rs"]
mod async_recording_spi;
#[cfg(feature = "async")]
#[path = "common/poll_support.rs"]
mod poll_support;

#[cfg(feature = "async")]
#[test]
fn repeated_commit_preserves_async_publication_state() {
    use async_recording_spi::AsyncRecordingConfig;
    use async_recording_spi::async_recording_file_system;
    use poll_support::ready;

    let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig::default());
    let target = Path::parse("/target").expect("path should parse");
    let mut writer = ready(filesystem.open_writer(&target, WriteOptions::default())).expect("writer should open");
    ready(writer.commit_async()).expect("first commit should succeed");

    let failure = ready(writer.commit_async()).expect_err("committed writer must reject a second commit");
    assert_eq!(FsErrorKind::InvalidState, failure.error().kind());
    assert_eq!(WriteFailureState::Published, failure.state());
    assert_eq!(WriterState::Committed, writer.state());
}

#[cfg(feature = "async")]
#[test]
fn retryable_async_commit_remains_retryable() {
    use async_recording_spi::AsyncRecordingConfig;
    use async_recording_spi::async_recording_file_system;
    use poll_support::ready;

    let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
        writer_commit_failure: Some(WriteFailureState::RetryableNotPublished),
        ..AsyncRecordingConfig::default()
    });
    let target = Path::parse("/target").expect("path should parse");
    let mut writer = ready(filesystem.open_writer(&target, WriteOptions::default())).expect("writer should open");
    let first = ready(writer.commit_async()).expect_err("configured commit should fail");
    assert_eq!(WriteFailureState::RetryableNotPublished, first.state());
    assert_eq!(WriterState::Open, writer.state());
    let retry = ready(writer.commit_async()).expect_err("retry should reach the provider");
    assert_eq!(WriteFailureState::RetryableNotPublished, retry.state());
    assert_eq!(WriterState::Open, writer.state());
    let _ = ready(writer.abort_async()).expect("abort should succeed");
    let repeated = ready(writer.commit_async()).expect_err("aborted writer must reject commit");
    assert_eq!(FsErrorKind::InvalidState, repeated.error().kind());
}

#[cfg(feature = "async")]
#[test]
fn repeated_commit_preserves_async_failed_publication_states() {
    use async_recording_spi::AsyncRecordingConfig;
    use async_recording_spi::async_recording_file_system;
    use poll_support::ready;

    for (provider_state, expected_writer_state) in [
        (WriteFailureState::NotPublished, WriterState::NotPublished),
        (WriteFailureState::Published, WriterState::Published),
        (WriteFailureState::Indeterminate, WriterState::Indeterminate),
    ] {
        let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
            writer_commit_failure: Some(provider_state),
            ..AsyncRecordingConfig::default()
        });
        let target = Path::parse("/target").expect("path should parse");
        let mut writer = ready(filesystem.open_writer(&target, WriteOptions::default())).expect("writer should open");
        let first = ready(writer.commit_async()).expect_err("configured commit should fail");
        assert_eq!(provider_state, first.state());
        assert_eq!(expected_writer_state, writer.state());

        let repeated = ready(writer.commit_async()).expect_err("non-open writer must reject commit");
        assert_eq!(FsErrorKind::InvalidState, repeated.error().kind());
        assert_eq!(provider_state, repeated.state());
        assert_eq!(expected_writer_state, writer.state());
    }
}
