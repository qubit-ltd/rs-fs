// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runtime registry for asynchronous filesystem providers.

use std::collections::{
    HashMap,
    HashSet,
};
use std::error::Error;
use std::fmt::{
    Debug,
    Display,
    Formatter,
    Result as FmtResult,
};
use std::sync::Arc;

use parking_lot::RwLock;
use qubit_spi::{
    FallbackPolicy,
    ProviderDescriptor,
    ProviderSelection,
    ProviderSelector,
};

use crate::{
    AsyncFileResource,
    AsyncFileSystem,
    AsyncFileSystemProvider,
    FileLocation,
    FileSystemConfig,
    FileSystemResolution,
    FsError,
    FsErrorKind,
    FsFuture,
    FsOperation,
    FsResult,
    FsUri,
};

/// Shared runtime registry of asynchronous filesystem providers.
///
/// Clones observe the same atomic registrations and default selection. A
/// resolution snapshots candidates before awaiting provider code, so registry
/// locks are never held across an await point.
#[derive(Clone)]
pub struct AsyncFileSystemRegistry {
    inner: Arc<RwLock<RegistryState>>,
}

struct RegistryState {
    entries: Vec<RegistryEntry>,
    selector_indices: HashMap<ProviderSelector, usize>,
    automatic_indices: Vec<usize>,
    default_selection: ProviderSelection,
}

#[derive(Clone)]
struct RegistryEntry {
    descriptor: ProviderDescriptor,
    provider: Arc<dyn AsyncFileSystemProvider>,
}

/// Ordered asynchronous provider failures retained as an [`FsError`] source.
struct AsyncProviderCreationFailures {
    attempts: Box<[FsError]>,
    exhausted: bool,
}

impl Debug for AsyncProviderCreationFailures {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncProviderCreationFailures")
            .field("attempt_count", &self.attempts.len())
            .field("exhausted", &self.exhausted)
            .finish()
    }
}

impl Display for AsyncProviderCreationFailures {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        if self.exhausted {
            write!(
                formatter,
                "no asynchronous filesystem provider succeeded after {} attempt(s)",
                self.attempts.len(),
            )?;
        } else {
            write!(
                formatter,
                "asynchronous filesystem provider fallback stopped after {} attempt(s)",
                self.attempts.len(),
            )?;
        }
        for (index, attempt) in self.attempts.iter().enumerate() {
            let provider = attempt
                .provider()
                .map_or("<unknown>", |provider| provider.as_str());
            write!(
                formatter,
                "; attempt {} ({provider}): {attempt}",
                index + 1,
            )?;
        }
        Ok(())
    }
}

impl Error for AsyncProviderCreationFailures {}

impl AsyncFileSystemRegistry {
    /// Registers an owned asynchronous filesystem provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - Provider moved into shared registry storage.
    ///
    /// # Errors
    ///
    /// Returns [`FsErrorKind::Conflict`] without mutation when the canonical
    /// ID or any alias is already registered.
    #[inline]
    pub fn register<P>(&self, provider: P) -> FsResult<()>
    where
        P: AsyncFileSystemProvider + 'static,
    {
        self.register_shared(Arc::new(provider))
    }

    /// Registers an already shared asynchronous filesystem provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - Shared provider retained by the registry.
    ///
    /// # Errors
    ///
    /// Returns [`FsErrorKind::Conflict`] without mutation when the canonical
    /// ID or any alias is already registered.
    pub fn register_shared(
        &self,
        provider: Arc<dyn AsyncFileSystemProvider>,
    ) -> FsResult<()> {
        let descriptor = provider.descriptor();
        let canonical_selector = ProviderSelector::from(descriptor.id());
        let mut state = self.inner.write();
        validate_available(&state, &canonical_selector)?;
        for alias in descriptor.aliases() {
            validate_available(&state, alias)?;
        }

        let index = state.entries.len();
        state.selector_indices.insert(canonical_selector, index);
        for alias in descriptor.aliases() {
            state.selector_indices.insert(alias.clone(), index);
        }
        state.entries.push(RegistryEntry {
            descriptor,
            provider,
        });
        let mut automatic_indices =
            (0..state.entries.len()).collect::<Vec<_>>();
        automatic_indices.sort_unstable_by(|left, right| {
            let left = &state.entries[*left].descriptor;
            let right = &state.entries[*right].descriptor;
            right
                .priority()
                .cmp(&left.priority())
                .then_with(|| left.id().cmp(right.id()))
        });
        state.automatic_indices = automatic_indices;
        Ok(())
    }

    /// Returns the selection used by [`Self::resolve_async`].
    ///
    /// # Returns
    ///
    /// An owned snapshot of the current default selection.
    #[inline]
    #[must_use]
    pub fn default_selection(&self) -> ProviderSelection {
        self.inner.read().default_selection.clone()
    }

    /// Replaces the selection used by future [`Self::resolve_async`] calls.
    ///
    /// # Arguments
    ///
    /// * `selection` - Validated selection and fallback policy.
    #[inline]
    pub fn set_default_selection(&self, selection: ProviderSelection) {
        self.inner.write().default_selection = selection;
    }

    /// Resolves a complete configuration using its explicit selection or URI
    /// scheme.
    ///
    /// # Arguments
    ///
    /// * `config` - URI, optional selection, options, and credential reference.
    ///
    /// # Returns
    ///
    /// A future resolving to a provider-decoded filesystem result.
    pub fn resolve_config_async<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>> {
        let selection = match config.selection() {
            Some(selection) => Ok(selection.clone()),
            None => ProviderSelection::named(config.uri().scheme().as_str())
                .map_err(|error| {
                    let message = error.to_string();
                    FsError::with_source(
                        FsErrorKind::ProviderUnavailable,
                        FsOperation::Provider,
                        &message,
                        error,
                    )
                }),
        };
        let candidates = selection.and_then(|selection| {
            self.snapshot_candidates(&selection)
                .map(|entries| (entries, selection.fallback_policy()))
        });
        create_from_snapshot(candidates, config)
    }

    /// Resolves complete configuration through a supplied selection.
    ///
    /// # Arguments
    ///
    /// * `selection` - Candidate selection and fallback policy.
    /// * `config` - Complete configuration passed unchanged to each provider.
    ///
    /// # Returns
    ///
    /// A future resolving to a provider-decoded filesystem result.
    pub fn resolve_selected_async<'a>(
        &'a self,
        selection: &ProviderSelection,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>> {
        let candidates = self
            .snapshot_candidates(selection)
            .map(|entries| (entries, selection.fallback_policy()));
        create_from_snapshot(candidates, config)
    }

    /// Resolves complete configuration through the current default selection.
    ///
    /// # Arguments
    ///
    /// * `config` - Complete configuration passed unchanged to each provider.
    ///
    /// # Returns
    ///
    /// A future resolving to a provider-decoded filesystem result.
    pub fn resolve_async<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>> {
        let selection = self.default_selection();
        let candidates = self
            .snapshot_candidates(&selection)
            .map(|entries| (entries, selection.fallback_policy()));
        create_from_snapshot(candidates, config)
    }

    /// Creates an asynchronous filesystem from complete configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Complete provider configuration.
    ///
    /// # Returns
    ///
    /// A future resolving to the created filesystem object.
    pub fn file_system_async<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, Arc<dyn AsyncFileSystem>> {
        Box::pin(async move {
            Ok(self
                .resolve_config_async(config)
                .await?
                .file_system()
                .clone())
        })
    }

    /// Resolves complete configuration into a bound asynchronous resource.
    ///
    /// # Arguments
    ///
    /// * `config` - Complete provider configuration.
    ///
    /// # Returns
    ///
    /// A future resolving to a filesystem-bound provider-local resource.
    pub fn resource_async<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, AsyncFileResource> {
        Box::pin(async move {
            let resolution = self.resolve_config_async(config).await?;
            let (fs, path, canonical_uri) = resolution.into_parts();
            let location = FileLocation::new(fs.info().id().clone(), path)
                .with_uri(canonical_uri);
            Ok(AsyncFileResource::from_location(fs, location))
        })
    }

    /// Creates an asynchronous filesystem from a URI-only configuration.
    ///
    /// # Arguments
    ///
    /// * `uri` - Secret-free resource URI.
    ///
    /// # Returns
    ///
    /// A future resolving to the created filesystem object.
    pub fn file_system_uri_async<'a>(
        &'a self,
        uri: &'a FsUri,
    ) -> FsFuture<'a, Arc<dyn AsyncFileSystem>> {
        Box::pin(async move {
            self.file_system_async(&FileSystemConfig::new(uri.clone()))
                .await
        })
    }

    /// Resolves a URI-only configuration into a bound async resource.
    ///
    /// # Arguments
    ///
    /// * `uri` - Secret-free resource URI.
    ///
    /// # Returns
    ///
    /// A future resolving to a provider-local resource.
    pub fn resource_uri_async<'a>(
        &'a self,
        uri: &'a FsUri,
    ) -> FsFuture<'a, AsyncFileResource> {
        Box::pin(async move {
            self.resource_async(&FileSystemConfig::new(uri.clone()))
                .await
        })
    }

    /// Returns canonical provider IDs in registration order.
    ///
    /// # Returns
    ///
    /// A point-in-time provider ID snapshot.
    #[inline]
    #[must_use]
    pub fn provider_ids(&self) -> Vec<String> {
        self.inner
            .read()
            .entries
            .iter()
            .map(|entry| entry.descriptor.id().as_str().to_owned())
            .collect()
    }

    fn snapshot_candidates(
        &self,
        selection: &ProviderSelection,
    ) -> FsResult<Vec<RegistryEntry>> {
        let state = self.inner.read();
        let indices = if selection.selectors().is_empty() {
            if state.automatic_indices.is_empty() {
                return Err(provider_unavailable(
                    "no asynchronous filesystem provider is registered",
                ));
            }
            state.automatic_indices.clone()
        } else {
            let mut seen = HashSet::new();
            let indices = selection
                .selectors()
                .iter()
                .filter_map(|selector| {
                    state.selector_indices.get(selector).copied()
                })
                .filter(|index| seen.insert(*index))
                .collect::<Vec<_>>();
            if indices.is_empty() {
                return Err(provider_unavailable(
                    "provider selection matched no asynchronous filesystem provider",
                ));
            }
            indices
        };
        Ok(indices
            .into_iter()
            .map(|index| state.entries[index].clone())
            .collect())
    }
}

impl Default for AsyncFileSystemRegistry {
    /// Creates an empty asynchronous provider registry.
    ///
    /// # Returns
    ///
    /// A registry using automatic selection by default.
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryState {
                entries: Vec::new(),
                selector_indices: HashMap::new(),
                automatic_indices: Vec::new(),
                default_selection: ProviderSelection::default(),
            })),
        }
    }
}

fn validate_available(
    state: &RegistryState,
    selector: &ProviderSelector,
) -> FsResult<()> {
    if state.selector_indices.contains_key(selector) {
        return Err(FsError::new(
            FsErrorKind::Conflict,
            FsOperation::Provider,
            "asynchronous filesystem provider selector is already registered",
        ));
    }
    Ok(())
}

/// Tries a snapshotted candidate sequence without holding registry locks.
fn create_from_snapshot<'a>(
    candidates: FsResult<(Vec<RegistryEntry>, FallbackPolicy)>,
    config: &'a FileSystemConfig,
) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>> {
    Box::pin(async move {
        let (candidates, fallback_policy) = candidates?;
        let candidate_count = candidates.len();
        let mut failures = Vec::new();
        for (index, entry) in candidates.into_iter().enumerate() {
            match entry.provider.create_configured_async(config).await {
                Ok(resolution) => return Ok(resolution),
                Err(error) => {
                    let error =
                        error.with_provider(entry.descriptor.id().clone());
                    let may_continue =
                        allows_fallback(fallback_policy, error.kind());
                    let exhausted = index + 1 == candidate_count;
                    failures.push(error);
                    if exhausted || !may_continue {
                        if failures.len() == 1 {
                            return Err(failures
                                .pop()
                                .expect("one provider failure was recorded"));
                        }
                        return Err(aggregate_creation_failures(
                            failures, exhausted,
                        ));
                    }
                }
            }
        }
        Err(provider_unavailable("provider selection had no candidates"))
    })
}

/// Reports whether one provider error admits another candidate attempt.
#[inline]
fn allows_fallback(policy: FallbackPolicy, kind: FsErrorKind) -> bool {
    if policy == FallbackPolicy::OnAnyError {
        true
    } else if policy == FallbackPolicy::OnAbsence {
        is_absence_error(kind)
    } else {
        false
    }
}

/// Builds one aggregate error without losing ordered provider failures.
fn aggregate_creation_failures(
    attempts: Vec<FsError>,
    exhausted: bool,
) -> FsError {
    debug_assert!(attempts.len() > 1);
    let kind = if attempts
        .iter()
        .all(|attempt| is_absence_error(attempt.kind()))
    {
        FsErrorKind::ProviderUnavailable
    } else {
        FsErrorKind::Other
    };
    let message = if exhausted {
        "no asynchronous filesystem provider succeeded"
    } else {
        "asynchronous filesystem provider fallback stopped"
    };
    FsError::with_source(
        kind,
        FsOperation::Provider,
        message,
        AsyncProviderCreationFailures {
            attempts: attempts.into_boxed_slice(),
            exhausted,
        },
    )
}

/// Reports whether an error denotes provider absence or unsupported service.
#[inline]
fn is_absence_error(kind: FsErrorKind) -> bool {
    matches!(
        kind,
        FsErrorKind::ProviderUnavailable
            | FsErrorKind::UnsupportedCapability
            | FsErrorKind::UnsupportedOperation
    )
}

#[inline]
fn provider_unavailable(message: &str) -> FsError {
    FsError::new(
        FsErrorKind::ProviderUnavailable,
        FsOperation::Provider,
        message,
    )
}
