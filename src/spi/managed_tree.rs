//! Owned tree-operation protocol declarations for managed providers.
//!
//! Providers validate staged content and publication requirements before
//! activating I/O. A request with an unsupported `Required` guarantee must
//! fail during preparation. Validation failures leave the staged-tree owner
//! responsible for cleanup; successful publication reports the achieved
//! atomicity and durability in [`TreePublishOutcome`].
use crate::{
    FsOperationFailure,
    directory::TreePublishOutcome,
    execution::{FsOwnedTreePublishRequest, FsPrepareFailure},
};
use qubit_operation::PreparedExternalOperation;
use std::{
    collections::BTreeSet,
    fmt::{Debug, Formatter, Result as FmtResult},
    sync::{Arc, Mutex, PoisonError},
    time::Instant,
};

/// Cleanup-owning staged directory used while validating and publishing a tree.
///
/// The provider keeps this value alive through validation. Validation errors
/// do not release the cleanup callback. It is released only after a
/// consistent successful publication outcome. When registered through
/// [`ManagedFsTreeSpi::commit_staged_directory`], failed cleanup returns this
/// owner to the operation resource scope for recovery and retry.
pub struct StagedDirectory {
    root: crate::path::Path,
    options: crate::metadata::TreeValidationOptions,
    state: Mutex<StagedDirectoryState>,
    cleanup: Mutex<Option<Box<dyn FnMut() -> crate::error::FsResult<()> + Send>>>,
    cleanup_resource_id: Mutex<Option<qubit_operation::ResourceId>>,
}

struct StagedDirectoryState {
    entries: usize,
    content_bytes: u64,
    normalized_paths: BTreeSet<String>,
    validated_version: Option<crate::metadata::ResourceVersion>,
    published: bool,
}

impl StagedDirectory {
    fn new(
        root: crate::path::Path,
        options: crate::metadata::TreeValidationOptions,
        cleanup: Box<dyn FnMut() -> crate::error::FsResult<()> + Send>,
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
            cleanup_resource_id: Mutex::new(None),
        }
    }

    /// Returns the logical root of this staging resource.
    pub fn root(&self) -> &crate::path::Path {
        &self.root
    }

    /// Returns whether `candidate` is the staging root or a descendant by
    /// path component. Text prefixes that end within a component do not match.
    #[must_use]
    pub fn contains_path(&self, candidate: &crate::path::Path) -> bool {
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
    pub fn record_validated_version(&self, version: crate::metadata::ResourceVersion) {
        lock(&self.state).validated_version = Some(version);
    }

    /// Rejects a changed or unversioned tree before a required atomic publish.
    pub fn check_publish_version(
        &self,
        current_version: Option<&crate::metadata::ResourceVersion>,
        requirements: &crate::metadata::TreePublishOptions,
    ) -> crate::error::FsResult<()> {
        if requirements.atomicity != crate::metadata::AtomicityRequirement::Required {
            return Ok(());
        }
        if lock(&self.state)
            .validated_version
            .as_ref()
            .is_none_or(|validated| current_version.is_none_or(|current| current != validated))
        {
            return Err(crate::error::FsError::new(
                crate::error::FsErrorKind::UnsupportedCapability,
                crate::error::FsOperation::ValidateProviderOutcome,
                "staged tree changed after validation; required atomic publication is unavailable",
            )
            .with_required_capability(crate::metadata::FileSystemCapability::AtomicTreeCopy));
        }
        Ok(())
    }

    /// Builds an owned publication request while retaining this cleanup owner.
    pub fn publish_request(
        &self,
        destination_root: crate::path::Path,
        publication: crate::metadata::TreePublishOptions,
        current_version: Option<&crate::metadata::ResourceVersion>,
        capabilities: &crate::metadata::FileSystemCapabilities,
    ) -> crate::error::FsResult<FsOwnedTreePublishRequest> {
        publication.validate_capabilities(capabilities)?;
        self.check_publish_version(current_version, &publication)?;
        Ok(FsOwnedTreePublishRequest {
            staging_root: self.root.clone(),
            destination_root,
            validation: self.options,
            publication,
        })
    }

    /// Validates one entry using actual measured content bytes.
    pub fn validate_entry(
        &self,
        relative_path: &str,
        normalized_path: &str,
        is_symlink: bool,
        measured_content_bytes: u64,
    ) -> crate::error::FsResult<()> {
        let mut state = lock(&self.state);
        self.options.validate_entry(
            relative_path,
            normalized_path,
            is_symlink,
            state.entries,
            state.content_bytes,
            measured_content_bytes,
            &mut state.normalized_paths,
        )?;
        state.entries += 1;
        state.content_bytes += measured_content_bytes;
        Ok(())
    }

    /// Validates one bounded batch before processing its records.
    pub fn validate_batch(&self, record_sizes: &[usize]) -> crate::error::FsResult<()> {
        self.options.validate_batch(record_sizes)
    }

    /// Checks the deadline and stop state before the next validation step.
    pub fn check_active(
        &self,
        now: Instant,
        deadline: Option<Instant>,
        stop_requested: bool,
    ) -> crate::error::FsResult<()> {
        self.options.check_active(now, deadline, stop_requested)
    }

    /// Retries cleanup while retaining responsibility after cleanup errors.
    pub fn cleanup(&self) -> crate::error::FsResult<()> {
        if lock(&self.state).published {
            lock(&self.cleanup).take();
            return Ok(());
        }
        let mut cleanup_owner = lock(&self.cleanup);
        let Some(cleanup) = cleanup_owner.as_mut() else {
            return Ok(());
        };
        cleanup()?;
        *cleanup_owner = None;
        Ok(())
    }

    /// Releases staging cleanup responsibility after successful publication.
    pub fn publication_succeeded(
        &self,
        outcome: &TreePublishOutcome,
        requirements: &crate::metadata::TreePublishOptions,
    ) -> crate::error::FsResult<()> {
        if !outcome.meets_requirements(requirements) {
            return Err(crate::error::FsError::new(
                crate::error::FsErrorKind::ProviderContractViolation,
                crate::error::FsOperation::ValidateProviderOutcome,
                "tree publication facts do not satisfy the request",
            ));
        }
        lock(&self.state).published = true;
        lock(&self.cleanup).take();
        Ok(())
    }

    fn set_cleanup_resource_id(&self, id: qubit_operation::ResourceId) {
        *lock(&self.cleanup_resource_id) = Some(id);
    }

    fn cleanup_failure(&self, error: crate::error::FsError) -> qubit_operation::CleanupFailure {
        let id = lock(&self.cleanup_resource_id)
            .as_ref()
            .expect("registered staged directory has a resource identity")
            .clone();
        qubit_operation::CleanupFailure {
            resource_id: id,
            stage: qubit_operation::CleanupStage::AsyncRelease,
            summary: qubit_operation::DiagnosticSummary::new(&error.to_string()),
            source: Some(Arc::new(error)),
        }
    }
}

impl Drop for StagedDirectory {
    fn drop(&mut self) {
        // Registered owners are cleaned by their ResourceScope policy. This
        // fallback covers a staging value rejected before registration.
        if let Some(cleanup) = lock(&self.cleanup).as_mut() {
            let _ = cleanup();
        }
    }
}

impl Debug for StagedDirectory {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("StagedDirectory")
            .field("root", &self.root)
            .field("entries", &lock(&self.state).entries)
            .field("content_bytes", &lock(&self.state).content_bytes)
            .field("validated", &lock(&self.state).validated_version.is_some())
            .finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

struct StagedDirectoryCleanup;

impl qubit_operation::AsyncCleanup<StagedDirectory> for StagedDirectoryCleanup {
    fn cleanup(
        self: Arc<Self>,
        resource: StagedDirectory,
        _context: qubit_operation::CleanupContext,
    ) -> qubit_operation::CleanupFuture<StagedDirectory> {
        Box::pin(async move {
            match resource.cleanup() {
                Ok(()) => qubit_operation::CleanupOutcome::Released,
                Err(error) => qubit_operation::CleanupOutcome::Failed {
                    failure: resource.cleanup_failure(error),
                    resource,
                },
            }
        })
    }
}
/// Object-safe prepared staged-tree publication operation.
pub trait PreparedFsTreePublish:
    PreparedExternalOperation<TreePublishOutcome, FsOperationFailure>
{
    /// Returns the original tree request while unclaimed.
    fn take_unclaimed_request(&self) -> Option<FsOwnedTreePublishRequest>;
}
/// Object-safe provider preparation interface for staged-tree publication.
pub trait ManagedFsTreeSpi: Send + Sync {
    /// Reserves operation-scope capacity before provider staging begins.
    fn reserve_staged_directory(
        &self,
        scope: &qubit_operation::ResourceScope,
    ) -> Result<qubit_operation::ResourceReservation, qubit_operation::ResourceCapacityError> {
        scope.reserve_resource()
    }

    /// Commits the staged owner and async cleanup policy into the reservation.
    ///
    /// The caller creates provider staging only after
    /// [`reserve_staged_directory`](Self::reserve_staged_directory) succeeds.
    /// If commit fails, the rejected value and callback are routed through the
    /// reservation's cleanup path before this method returns an error.
    fn commit_staged_directory(
        &self,
        reservation: qubit_operation::ResourceReservation,
        root: crate::path::Path,
        options: crate::metadata::TreeValidationOptions,
        cleanup: Box<dyn FnMut() -> crate::error::FsResult<()> + Send>,
    ) -> crate::error::FsResult<qubit_operation::ResourceRef<StagedDirectory>> {
        let owner = StagedDirectory::new(root, options, cleanup);
        match reservation.commit_with(owner, StagedDirectoryCleanup) {
            Ok(resource) => {
                resource.set_cleanup_resource_id(resource.id());
                Ok(resource)
            }
            Err(error) => {
                let (reason, owner, cleanup, reservation) = error.into_parts();
                let resource_id = reservation.retain_with_for_cleanup(owner, cleanup);
                let message = format!(
                    "staging owner registration failed ({reason:?}); cleanup retained as {resource_id:?}"
                );
                Err(crate::error::FsError::new(
                    crate::error::FsErrorKind::InvalidState,
                    crate::error::FsOperation::CreateTemp,
                    &message,
                ))
            }
        }
    }

    /// Prepares publication without starting tree I/O.
    ///
    /// Implementations must reject unsupported required guarantees before
    /// activating any I/O. They must not infer atomic tree publication from
    /// an object-prefix operation. Any validation failure leaves cleanup with
    /// the existing staging owner.
    fn prepare_tree_publish(
        &self,
        request: FsOwnedTreePublishRequest,
    ) -> Result<Arc<dyn PreparedFsTreePublish>, FsPrepareFailure<FsOwnedTreePublishRequest>>;
}
