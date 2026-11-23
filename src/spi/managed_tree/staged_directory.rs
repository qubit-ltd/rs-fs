// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Cleanup-owning staged-directory validation resource.

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::time::Instant;

use qubit_operation::CleanupFailure;
use qubit_operation::CleanupStage;
use qubit_operation::DiagnosticSummary;
use qubit_operation::ResourceId;

use super::internal::StagedCleanupIdentity;
use super::internal::StagedDirectoryState;
use crate::directory::TreePublishOutcome;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::execution::FsOwnedTreePublishRequest;
use crate::metadata::AtomicityRequirement;
use crate::metadata::FileSystemCapabilities;
use crate::metadata::FileSystemCapability;
use crate::metadata::ResourceVersion;
use crate::metadata::TreeEntryValidationState;
use crate::metadata::TreePublishOptions;
use crate::metadata::TreeValidationOptions;
use crate::path::Path;

/// Future returned by a provider's staged-directory cleanup callback.
pub(in crate::spi::managed_tree) type StagedCleanupFuture = Pin<Box<dyn Future<Output = FsResult<()>> + Send>>;
/// Callback that asynchronously releases the provider's staged directory.
pub(in crate::spi::managed_tree) type StagedCleanupCallback = Box<dyn FnMut() -> StagedCleanupFuture + Send>;

/// Cleanup-owning staged directory used while validating and publishing a tree.
///
/// The provider keeps this value alive through validation. Validation errors
/// do not release the cleanup callback. It is released only after a
/// consistent successful publication outcome. When registered through
/// [`crate::spi::ManagedFsTreeSpi::commit_staged_directory`], failed cleanup
/// returns this owner to the operation resource scope for recovery and retry.
///
/// # Examples
///
/// ```
/// use qubit_fs::spi::StagedDirectory;
///
/// fn verify_root(staged: &StagedDirectory) {
///     assert!(staged.contains_path(staged.root()));
/// }
/// ```
#[must_use]
pub struct StagedDirectory {
    /// Logical root under which staged tree entries are addressed.
    root: Path,
    /// Limits and path requirements applied during validation.
    options: TreeValidationOptions,
    /// Counts, normalized paths, and validation version for this staging owner.
    state: Mutex<StagedDirectoryState>,
    /// Provider callback that releases staging resources after publication.
    cleanup: Mutex<Option<StagedCleanupCallback>>,
    /// Operation resource identity, published by registration before cleanup
    /// failure can be reported.
    cleanup_resource_id: Arc<StagedCleanupIdentity>,
    /// Test synchronization point used to observe request construction under
    /// the validation-state lock.
    #[cfg(test)]
    publish_request_observer: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl StagedDirectory {
    /// Creates a staging owner with cleanup responsibility and empty validation
    /// state.
    ///
    /// # Parameters
    /// - `root`: Logical path of the staged tree.
    /// - `options`: Validation limits and requirements.
    /// - `cleanup`: Callback that releases provider staging resources.
    ///
    /// # Returns
    /// A staging owner ready to validate entries.
    pub(in crate::spi::managed_tree) fn new(
        root: Path,
        options: TreeValidationOptions,
        cleanup: StagedCleanupCallback,
    ) -> Self {
        Self {
            root,
            options,
            state: Mutex::new(StagedDirectoryState {
                entries: 0,
                content_bytes: 0,
                normalized_paths: BTreeSet::new(),
                validated_version: None,
                published: false,
            }),
            cleanup: Mutex::new(Some(cleanup)),
            cleanup_resource_id: Arc::new(StagedCleanupIdentity::new()),
            #[cfg(test)]
            publish_request_observer: Mutex::new(None),
        }
    }

    /// Returns the logical root of this staging resource.
    ///
    /// # Returns
    /// The borrowed staging root path.
    #[must_use]
    #[inline]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns whether `candidate` is the staging root or a descendant by
    /// path component. Text prefixes that end within a component do not match.
    ///
    /// # Parameters
    /// - `candidate`: Path to check against this staging root.
    ///
    /// # Returns
    /// `true` if the path has matching semantics and is the root or a component
    /// descendant.
    #[must_use]
    #[inline]
    pub fn contains_path(&self, candidate: &Path) -> bool {
        if self.root.semantics() != candidate.semantics() {
            return false;
        }
        if self.root == *candidate {
            return true;
        }
        let root = self.root.as_str();
        let candidate = candidate.as_str();
        let Some(remainder) = candidate.strip_prefix(root) else {
            return false;
        };
        root.ends_with('/') || remainder.starts_with('/')
    }

    /// Records the provider version observed after validating this staged tree.
    ///
    /// # Parameters
    /// - `version`: Provider version observed for the validated staged content.
    pub fn record_validated_version(&self, version: ResourceVersion) {
        lock(&self.state).validated_version = Some(version);
    }

    /// Rejects a changed or unversioned tree before a required atomic publish.
    ///
    /// # Parameters
    /// - `current_version`: Version currently observed for the destination
    ///   source.
    /// - `requirements`: Requested publication guarantees.
    ///
    /// # Returns
    /// `Ok(())` when no required atomic version check is needed or versions
    /// match; otherwise an error describing the unmet capability.
    ///
    /// # Errors
    /// Returns an unsupported-capability error when a required atomic publish
    /// has no matching validated version.
    pub fn check_publish_version(
        &self,
        current_version: Option<&ResourceVersion>,
        requirements: &TreePublishOptions,
    ) -> FsResult<()> {
        let state = lock(&self.state);
        Self::checked_publish_version(&state, current_version, requirements).map(|_| ())
    }

    /// Builds an owned publication request while retaining this cleanup owner.
    ///
    /// # Parameters
    /// - `destination_root`: Destination for the published tree.
    /// - `publication`: Atomicity and durability requirements.
    /// - `current_version`: Current version used to validate required
    ///   atomicity.
    /// - `capabilities`: Provider capabilities used to validate the request.
    ///
    /// # Returns
    /// An owned request carrying the staging root and recorded validation
    /// facts.
    ///
    /// # Errors
    /// Returns an error when capabilities or required version checks fail.
    pub fn publish_request(
        &self,
        destination_root: Path,
        publication: TreePublishOptions,
        current_version: Option<&ResourceVersion>,
        capabilities: &FileSystemCapabilities,
    ) -> FsResult<FsOwnedTreePublishRequest> {
        publication.validate_capabilities(capabilities)?;
        let state = lock(&self.state);
        let validated_version = Self::checked_publish_version(&state, current_version, &publication)?;
        #[cfg(test)]
        if let Some(observer) = lock(&self.publish_request_observer).as_ref() {
            observer();
        }
        let validated_version = validated_version.cloned();
        Ok(FsOwnedTreePublishRequest {
            staging_root: self.root.clone(),
            destination_root,
            validation: self.options,
            validated_version,
            publication,
        })
    }

    /// Ensures required atomic publication uses the version captured at
    /// validation.
    ///
    /// # Parameters
    /// - `state`: Staging validation state protected by the caller's lock.
    /// - `current_version`: Current destination version, if one is available.
    /// - `requirements`: Publication atomicity requirements to enforce.
    ///
    /// # Returns
    /// The version validated for this staging resource when requirements pass.
    ///
    /// # Errors
    /// Returns an unsupported-capability error when the required version check
    /// cannot establish atomic publication.
    fn checked_publish_version<'a>(
        state: &'a StagedDirectoryState,
        current_version: Option<&ResourceVersion>,
        requirements: &TreePublishOptions,
    ) -> FsResult<Option<&'a ResourceVersion>> {
        let validated_version = state.validated_version.as_ref();
        if requirements.atomicity == AtomicityRequirement::Required
            && validated_version.is_none_or(|validated| current_version.is_none_or(|current| current != validated))
        {
            return Err(FsError::new(
                FsErrorKind::UnsupportedCapability,
                FsOperation::ValidateProviderOutcome,
                "staged tree changed after validation; required atomic publication is unavailable",
            )
            .with_required_capability(FileSystemCapability::AtomicTreeCopy));
        }
        Ok(validated_version)
    }

    /// Validates one entry using actual measured content bytes.
    ///
    /// # Parameters
    /// - `relative_path`: Provider-relative entry path.
    /// - `normalized_path`: Canonical path used for duplicate detection.
    /// - `is_symlink`: Whether the entry is a symbolic link.
    /// - `measured_content_bytes`: Actual content size measured by the
    ///   provider.
    ///
    /// # Returns
    /// `Ok(())` after recording an entry that satisfies the validation options.
    ///
    /// # Errors
    /// Returns the first violated tree-validation requirement.
    pub fn validate_entry(
        &self,
        relative_path: &str,
        normalized_path: &str,
        is_symlink: bool,
        measured_content_bytes: u64,
    ) -> FsResult<()> {
        let mut state = lock(&self.state);
        self.options.validate_entry(
            relative_path,
            normalized_path,
            is_symlink,
            &mut TreeEntryValidationState {
                entries_seen: state.entries,
                content_bytes_seen: state.content_bytes,
                measured_content_bytes,
                normalized_paths: &mut state.normalized_paths,
            },
        )?;
        state.entries += 1;
        state.content_bytes += measured_content_bytes;
        Ok(())
    }

    /// Validates one bounded batch before processing its records.
    ///
    /// # Parameters
    /// - `record_sizes`: Sizes of records in the next batch.
    ///
    /// # Returns
    /// `Ok(())` when the batch fits configured limits.
    ///
    /// # Errors
    /// Returns a validation error when a record or batch exceeds configured
    /// limits.
    pub fn validate_batch(&self, record_sizes: &[usize]) -> FsResult<()> {
        self.options.validate_batch(record_sizes)
    }

    /// Checks the deadline and stop state before the next validation step.
    ///
    /// # Parameters
    /// - `now`: Current monotonic time.
    /// - `deadline`: Optional operation deadline.
    /// - `stop_requested`: Whether cancellation has been requested.
    ///
    /// # Returns
    /// `Ok(())` while validation may continue.
    ///
    /// # Errors
    /// Returns a deadline or cancellation error when validation must stop.
    pub fn check_active(&self, now: Instant, deadline: Option<Instant>, stop_requested: bool) -> FsResult<()> {
        self.options.check_active(now, deadline, stop_requested)
    }

    /// Releases staging cleanup responsibility after successful publication.
    ///
    /// # Parameters
    /// - `outcome`: Provider-confirmed publication facts.
    /// - `requirements`: Requested publication guarantees.
    ///
    /// # Returns
    /// `Ok(())` when the outcome satisfies the request and cleanup is released.
    ///
    /// # Errors
    /// Returns a provider-contract violation when the outcome misses a
    /// requirement.
    pub fn publication_succeeded(
        &self,
        outcome: &TreePublishOutcome,
        requirements: &TreePublishOptions,
    ) -> FsResult<()> {
        if !outcome.meets_requirements(requirements) {
            return Err(FsError::new(
                FsErrorKind::ProviderContractViolation,
                FsOperation::ValidateProviderOutcome,
                "tree publication facts do not satisfy the request",
            ));
        }
        lock(&self.state).published = true;
        lock(&self.cleanup).take();
        Ok(())
    }

    /// Retries cleanup while retaining responsibility after cleanup errors.
    ///
    /// # Returns
    /// `Ok(())` after cleanup is confirmed or already unnecessary.
    ///
    /// # Errors
    /// Returns the provider cleanup failure while retaining the callback for
    /// retry.
    pub(in crate::spi::managed_tree) async fn cleanup(&self) -> FsResult<()> {
        if lock(&self.state).published {
            lock(&self.cleanup).take();
            return Ok(());
        }
        let attempt = {
            let mut cleanup_owner = lock(&self.cleanup);
            cleanup_owner.as_mut().map(|cleanup| cleanup())
        };
        let Some(attempt) = attempt else {
            return Ok(());
        };
        attempt.await?;
        lock(&self.cleanup).take();
        Ok(())
    }

    /// Associates the registered staging resource identity with this owner.
    ///
    /// # Parameters
    /// - `id`: Resource identity assigned by the operation scope.
    pub(in crate::spi::managed_tree) fn cleanup_identity(&self) -> Arc<StagedCleanupIdentity> {
        Arc::clone(&self.cleanup_resource_id)
    }

    /// Builds a cleanup failure tied to the registered resource identity.
    ///
    /// # Parameters
    /// - `error`: Provider cleanup error to preserve.
    /// - `resource_id`: Identity assigned by the operation scope.
    ///
    /// # Returns
    /// A resource-scoped cleanup failure retaining the source error.
    pub(in crate::spi::managed_tree) fn cleanup_failure(
        &self,
        error: FsError,
        resource_id: ResourceId,
    ) -> CleanupFailure {
        CleanupFailure {
            resource_id,
            stage: CleanupStage::AsyncRelease,
            summary: DiagnosticSummary::new(&error.to_string()),
            source: Some(Arc::new(error)),
        }
    }
}

impl Debug for StagedDirectory {
    /// Formats the staging root and validation progress without exposing
    /// callbacks.
    ///
    /// # Parameters
    /// - `self`: Staged owner to describe.
    /// - `formatter`: Formatter receiving the diagnostic structure.
    ///
    /// # Returns
    /// The formatter result, including any formatting error.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        let (entries, content_bytes, validated) = {
            let state = lock(&self.state);
            (state.entries, state.content_bytes, state.validated_version.is_some())
        };
        formatter
            .debug_struct("StagedDirectory")
            .field("root", &self.root)
            .field("entries", &entries)
            .field("content_bytes", &content_bytes)
            .field("validated", &validated)
            .finish_non_exhaustive()
    }
}

/// Acquires a mutex while recovering its inner value after poisoning.
///
/// # Type Parameters
/// - `T`: State protected by the mutex.
///
/// # Parameters
/// - `mutex`: State mutex used by the staged-directory owner.
///
/// # Returns
/// The held mutex guard, even if a previous operation poisoned the lock.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Condvar;
    use std::sync::Mutex;
    use std::sync::PoisonError;
    use std::sync::TryLockError;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::StagedDirectory;
    use super::lock;
    use crate::Path;
    use crate::metadata::AtomicityRequirement;
    use crate::metadata::DurabilityRequirement;
    use crate::metadata::FileSystemCapabilities;
    use crate::metadata::FileSystemCapability;
    use crate::metadata::TreePublishOptions;
    use crate::metadata::TreeValidationOptions;

    #[test]
    fn test_publish_request_keeps_the_version_compared_during_an_interleaved_update() {
        let staged = Arc::new(StagedDirectory::new(
            Path::parse("/stage").unwrap(),
            TreeValidationOptions::default(),
            Box::new(|| Box::pin(async { Ok(()) })),
        ));
        let required = TreePublishOptions {
            atomicity: AtomicityRequirement::Required,
            durability: DurabilityRequirement::NotRequired,
        };
        let capabilities = FileSystemCapabilities::new().with_conditional(FileSystemCapability::AtomicTreeCopy);
        staged.record_validated_version("version-before-update".into());

        let (lock_acquired_tx, lock_acquired_rx) = mpsc::channel();
        let released = Arc::new((Mutex::new(false), Condvar::new()));
        let observed_lock = Arc::new(AtomicBool::new(false));
        let observer_release = Arc::clone(&released);
        *lock(&staged.publish_request_observer) = Some(Arc::new(move || {
            lock_acquired_tx
                .send(())
                .expect("the updater should be waiting for the lock observation");
            let (released_lock, condition) = &*observer_release;
            let ready = lock(released_lock);
            let (ready, timeout) = condition
                .wait_timeout_while(ready, Duration::from_secs(5), |ready| !*ready)
                .unwrap_or_else(PoisonError::into_inner);
            assert!(
                *ready && !timeout.timed_out(),
                "the updater should release the observer"
            );
        }));

        let updater_staged = Arc::clone(&staged);
        let updater_released = Arc::clone(&released);
        let updater_observed_lock = Arc::clone(&observed_lock);
        let updater = thread::spawn(move || {
            if lock_acquired_rx.recv_timeout(Duration::from_secs(5)).is_err() {
                return;
            }
            let blocked = matches!(updater_staged.state.try_lock(), Err(TryLockError::WouldBlock));
            updater_observed_lock.store(blocked, Ordering::SeqCst);
            let (released_lock, condition) = &*updater_released;
            *lock(released_lock) = true;
            condition.notify_one();
            updater_staged.record_validated_version("version-after-update".into());
        });

        let request = staged
            .publish_request(
                Path::parse("/destination").unwrap(),
                required,
                Some(&"version-before-update".into()),
                &capabilities,
            )
            .expect("the compared version should be captured in the request");
        updater.join().expect("the concurrent version update should finish");
        assert!(observed_lock.load(Ordering::SeqCst));
        assert_eq!(request.validated_version, Some("version-before-update".into()));

        assert_eq!(
            lock(&staged.state).validated_version,
            Some("version-after-update".into())
        );
    }
}
