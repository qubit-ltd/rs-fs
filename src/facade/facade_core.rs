// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared immutable state and deterministic policy for filesystem facades.

use std::sync::Arc;

use qubit_budget::InsufficientBudgetError;
use qubit_budget::ResourceBudget;

use crate::copy::CopyAssessment;
use crate::copy::CopyExecutionRoute;
use crate::copy::CopyOptions;
use crate::copy::FallbackRejection;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::facade::internal::ByteBudget;
use crate::facade::internal::FileSystemResource;
use crate::metadata::FileSystemCapability;
use crate::metadata::FileSystemProperties;
use crate::path::Path;
use crate::path::PathSemantics;
use crate::spi::ProviderOperation;
use crate::spi::ProviderOperations;
use crate::spi::ProviderProperties;
use crate::write::WriteOptions;

/// Immutable provider state and deterministic preflight shared by facades.
pub(crate) struct FacadeCore {
    /// Effective application-facing properties derived from the provider.
    properties: Arc<FileSystemProperties>,
    /// Concrete provider operations captured with the property snapshot.
    provider_operations: ProviderOperations,
}

impl FacadeCore {
    /// Fixed I/O chunk used by bounded prefix reads.
    pub(crate) const PREFIX_BUFFER_SIZE: usize = 8192;

    /// Validates and captures one provider property snapshot.
    ///
    /// # Parameters
    ///
    /// - `provider`: Validated provider property snapshot containing identity,
    ///   operations, capabilities, and limits used to derive the facade cache.
    ///
    /// # Errors
    /// Returns the provider-property validation error when the declared
    /// capabilities and concrete operations are inconsistent.
    pub(crate) fn new(provider: ProviderProperties) -> FsResult<Self> {
        let provider_operations = provider.operations();
        let properties = FileSystemProperties::from_provider(&provider)?;
        Ok(Self {
            properties: Arc::new(properties),
            provider_operations,
        })
    }

    /// Returns the cached effective application-facing properties.
    #[inline]
    pub(crate) fn properties(&self) -> &FileSystemProperties {
        &self.properties
    }

    /// Reports whether the captured provider exposes a concrete operation.
    ///
    /// # Parameters
    ///
    /// - `operation`: Filesystem operation used for validation or error
    ///   context.
    #[inline]
    pub(crate) fn provider_supports(&self, operation: ProviderOperation) -> bool {
        self.provider_operations.supports(operation)
    }

    /// Validates one logical path against the cached provider snapshot.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `operation`: Filesystem operation used for validation or error
    ///   context.
    ///
    /// # Errors
    /// Returns an enriched invalid-path or resource-limit error when the path
    /// semantics, form, constraints, or limits do not match the filesystem.
    pub(crate) fn validate_path(&self, path: &Path, operation: FsOperation) -> FsResult<()> {
        self.properties.validate_path(path, operation)
    }

    /// Validates the optional parent used for temporary resource creation.
    ///
    /// # Parameters
    ///
    /// - `parent`: Optional logical parent path for temporary resource
    ///   creation.
    ///
    /// # Errors
    /// Returns an enriched path-validation error when `parent` is present and
    /// does not satisfy the cached filesystem rules.
    pub(crate) fn validate_temp_parent(&self, parent: Option<&Path>) -> FsResult<()> {
        parent.map_or(Ok(()), |path| self.validate_path(path, FsOperation::CreateTemp))
    }

    /// Validates a temporary resource's logical location under an explicit
    /// parent when the provider's path semantics define that relationship.
    ///
    /// # Parameters
    ///
    /// - `requested_parent`: Optional parent location supplied for temporary
    ///   creation.
    /// - `returned_path`: Logical path returned by the provider for the
    ///   temporary resource.
    pub(crate) fn validate_temp_location(&self, requested_parent: Option<&Path>, returned_path: &Path) -> FsResult<()> {
        let Some(parent) = requested_parent else {
            return Ok(());
        };
        let valid = match self.properties.info().path_semantics() {
            PathSemantics::Hierarchical => {
                (!parent.is_absolute() || returned_path.is_absolute())
                    && parent
                        .components()
                        .zip(returned_path.components())
                        .all(|(parent_component, returned_component)| parent_component == returned_component)
                    && returned_path.components().count() > parent.components().count()
            }
            PathSemantics::ObjectKey => returned_path.as_str().starts_with(parent.as_str()),
            PathSemantics::ProviderSpecific => true,
        };
        if valid {
            Ok(())
        } else {
            Err(self.contract_error(
                parent,
                FsOperation::ValidateProviderOutcome,
                "provider returned a temporary resource outside the requested parent location",
            ))
        }
    }

    /// Validates a writer path, options, and required write capability.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `options`: Write behavior and guarantees to validate before provider
    ///   work begins.
    pub(crate) fn validate_write_request(&self, path: &Path, options: &WriteOptions) -> FsResult<()> {
        self.validate_path(path, FsOperation::OpenWriter)?;
        options
            .validate_against(self.properties.capabilities())
            .map_err(|error| self.enrich(error, Some(path), FsOperation::OpenWriter))?;
        self.require(FileSystemCapability::Write, FsOperation::OpenWriter, Some(path))
    }

    /// Assesses copy routes from the immutable provider snapshot without I/O.
    ///
    /// # Parameters
    ///
    /// - `source`: Logical path of the source resource.
    /// - `target`: Destination logical path affected by the operation.
    /// - `options`: Copy route, conflict, consistency, and metadata guarantees
    ///   requested by the caller.
    ///
    /// # Errors
    /// Returns an invalid-options or requirement error when the paths or copy
    /// options cannot be served by either the provider or the stream fallback.
    pub(crate) fn assess_copy(&self, source: &Path, target: &Path, options: &CopyOptions) -> FsResult<CopyAssessment> {
        self.validate_path(source, FsOperation::Copy)?;
        self.validate_path(target, FsOperation::Copy)?;
        if source == target {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::Copy,
                "copy source and target must differ",
            )
            .with_missing_context(source, Some(target), self.properties.info().provider_id()));
        }
        options
            .validate_against(self.properties.capabilities())
            .map_err(|error| {
                self.enrich(error, Some(source), FsOperation::Copy)
                    .with_target(target.clone())
            })?;
        if options.max_entries() == Some(0) {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::Copy,
                "copy entry limit must be greater than zero",
            )
            .with_missing_context(source, Some(target), self.properties.info().provider_id()));
        }
        let mut rejection = crate::copy::fallback_rejection(options, self.properties.symlink_policy());
        if rejection.is_none() {
            rejection = self.stream_copy_primitive_rejection();
        }
        let provider_copy = self.provider_operations.supports(ProviderOperation::TryCopy);
        match (provider_copy, rejection) {
            (true, reason @ None) => Ok(CopyAssessment::new(CopyExecutionRoute::ProviderThenStream, reason)),
            (true, reason @ Some(_)) => Ok(CopyAssessment::new(CopyExecutionRoute::ProviderOnly, reason)),
            (false, None) => Ok(CopyAssessment::new(CopyExecutionRoute::StreamOnly, None)),
            (false, Some(_)) => Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::Copy,
                "copy has no available provider or stream execution path",
            )
            .with_missing_context(source, Some(target), self.properties.info().provider_id())),
        }
    }

    /// Returns the first missing primitive required by streamed copy.
    ///
    /// This checks only the cached provider snapshot and performs no I/O.
    /// The order is Read, Write, then Stat to preserve assessment behavior.
    fn stream_copy_primitive_rejection(&self) -> Option<FallbackRejection> {
        if !self.properties.capabilities().supports(FileSystemCapability::Read)
            || !self.provider_operations.supports(ProviderOperation::OpenReader)
        {
            Some(FallbackRejection::MissingRead)
        } else if !self.properties.capabilities().supports(FileSystemCapability::Write)
            || !self.provider_operations.supports(ProviderOperation::OpenWriter)
        {
            Some(FallbackRejection::MissingWrite)
        } else if !self.provider_operations.supports(ProviderOperation::Stat) {
            Some(FallbackRejection::MissingStat)
        } else {
            None
        }
    }

    /// Requires the cached primitives needed before streamed copy can do I/O.
    ///
    /// # Parameters
    ///
    /// - `source`: Requested source path.
    /// - `target`: Requested destination path.
    ///
    /// # Errors
    /// Returns `RequirementNotMet` with source, target, and provider context
    /// when any required reader, writer, or stat primitive is unavailable.
    pub(crate) fn require_stream_copy_primitives(&self, source: &Path, target: &Path) -> FsResult<()> {
        if self.stream_copy_primitive_rejection().is_none() {
            return Ok(());
        }
        Err(FsError::new(
            FsErrorKind::RequirementNotMet,
            FsOperation::Copy,
            "stream copy requires available metadata, reader, and writer primitives",
        )
        .with_missing_context(source, Some(target), self.properties.info().provider_id()))
    }

    /// Requires one capability before an operation can create provider I/O.
    ///
    /// `path` may be absent for operations that have no natural path context.
    ///
    /// # Parameters
    ///
    /// - `capability`: Capability required before provider work can begin.
    /// - `operation`: Filesystem operation used for validation or error
    ///   context.
    /// - `path`: Logical path of the resource involved in this operation.
    ///
    /// # Errors
    /// Returns an enriched unsupported-capability error when the cached
    /// properties do not advertise `capability`.
    pub(crate) fn require(
        &self,
        capability: FileSystemCapability,
        operation: FsOperation,
        path: Option<&Path>,
    ) -> FsResult<()> {
        if self.properties.capabilities().supports(capability) {
            Ok(())
        } else {
            let error = FsError::new(
                FsErrorKind::UnsupportedCapability,
                operation,
                "filesystem capability is not supported",
            )
            .with_required_capability(capability)
            .with_missing_provider(self.properties.info().provider_id());
            Err(match path {
                Some(path) => error.with_path(path.clone()),
                None => error,
            })
        }
    }

    /// Adds missing operation, optional path, and provider context to an error.
    ///
    /// Existing provider-supplied context is preserved. `None` does not invent
    /// a path for pathless operations.
    ///
    /// # Parameters
    ///
    /// - `error`: Filesystem error whose kind, source, and established facts
    ///   are retained.
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `operation`: Filesystem operation used for validation or error
    ///   context.
    pub(crate) fn enrich(&self, error: FsError, path: Option<&Path>, operation: FsOperation) -> FsError {
        let error = error
            .with_operation(operation)
            .with_missing_provider(self.properties.info().provider_id());
        match path {
            Some(path) => error.with_missing_context(path, None, self.properties.info().provider_id()),
            None => error,
        }
    }

    /// Builds a provider-contract error bound to a requested path.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `operation`: Filesystem operation used for validation or error
    ///   context.
    /// - `message`: Diagnostic message for the constructed filesystem error.
    pub(crate) fn contract_error(&self, path: &Path, operation: FsOperation, message: &str) -> FsError {
        FsError::new(FsErrorKind::ProviderContractViolation, operation, message)
            .with_path(path.clone())
            .with_provider(self.properties.info().provider_id())
    }

    /// Creates a byte budget with the supplied inclusive limit.
    ///
    /// # Parameters
    ///
    /// - `resource`: Resource category tracked by this byte budget.
    /// - `maximum`: Inclusive upper bound for this byte budget.
    #[inline]
    pub(crate) fn byte_budget(resource: FileSystemResource, maximum: u64) -> ByteBudget {
        ResourceBudget::new(resource, maximum)
    }

    /// Converts a platform-sized I/O count to the budget quantity type.
    ///
    /// # Parameters
    ///
    /// - `value`: Platform-sized I/O byte count to convert into resource
    ///   accounting units.
    /// - `operation`: Filesystem operation used for validation or error
    ///   context.
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `provider`: Provider identifier attached to contextual errors.
    ///
    /// # Errors
    /// Returns a contextual resource-limit error when `value` cannot be
    /// represented by the public `u64` accounting domain.
    #[inline]
    pub(crate) fn quantity_from_usize(
        value: usize,
        operation: FsOperation,
        path: &Path,
        provider: &str,
    ) -> Result<u64, FsError> {
        u64::try_from(value).map_err(|error| {
            FsError::with_source(
                FsErrorKind::ResourceLimitExceeded,
                operation,
                "I/O byte count cannot be represented by the resource budget",
                error,
            )
            .with_path(path.clone())
            .with_provider(provider)
        })
    }

    /// Converts a budget failure into a contextual filesystem error.
    ///
    /// # Parameters
    ///
    /// - `error`: Insufficient-budget error retained as the source of the new
    ///   `ResourceLimitExceeded` filesystem error.
    /// - `operation`: Filesystem operation used for validation or error
    ///   context.
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `provider`: Provider identifier attached to contextual errors.
    /// - `message`: Diagnostic message for the constructed filesystem error.
    #[inline]
    pub(crate) fn budget_error(
        error: InsufficientBudgetError<FileSystemResource, u64>,
        operation: FsOperation,
        path: &Path,
        provider: &str,
        message: &'static str,
    ) -> FsError {
        FsError::with_source(FsErrorKind::ResourceLimitExceeded, operation, message, error)
            .with_path(path.clone())
            .with_provider(provider)
    }

    /// Returns the next bounded read length for an accumulated prefix.
    ///
    /// # Parameters
    ///
    /// - `accumulated`: Bytes already accumulated in the prefix buffer.
    /// - `maximum`: Inclusive upper bound for this byte budget.
    #[inline]
    pub(crate) fn next_prefix_read_len(accumulated: usize, maximum: usize) -> usize {
        maximum.saturating_sub(accumulated).min(Self::PREFIX_BUFFER_SIZE)
    }
}

#[cfg(test)]
mod tests {
    use super::FacadeCore;
    use crate::error::FsEffectState;
    use crate::error::FsError;
    use crate::error::FsErrorKind;
    use crate::error::FsOperation;
    use crate::metadata::FileSystemCapabilities;
    use crate::metadata::FileSystemId;
    use crate::metadata::FileSystemInfo;
    use crate::metadata::FileSystemLimits;
    use crate::metadata::SymlinkPolicy;
    use crate::path::Path;
    use crate::path::PathConstraints;
    use crate::path::PathSemantics;
    use crate::spi::ProviderOperations;
    use crate::spi::ProviderProperties;

    fn core() -> FacadeCore {
        let properties = ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("facade-core-test").expect("test id should be valid"),
                "facade-core-test",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new(),
            FileSystemCapabilities::new(),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("test provider properties should be valid");
        FacadeCore::new(properties).expect("facade core should construct")
    }

    #[test]
    fn test_enrich_preserves_source_and_effect_while_adding_context() {
        let core = core();
        let path = Path::parse("/resource").expect("test path should parse");
        let original = FsError::with_source(
            FsErrorKind::Io,
            FsOperation::Other,
            "provider operation failed",
            std::io::Error::other("injected source"),
        )
        .with_effect_state(FsEffectState::PartiallyApplied);

        let error = core.enrich(original, Some(&path), FsOperation::Delete);

        assert_eq!(FsErrorKind::Io, error.kind());
        assert_eq!(FsOperation::Delete, error.operation());
        assert_eq!(Some("facade-core-test"), error.provider());
        assert_eq!(Some(&path), error.path());
        assert_eq!(Some(FsEffectState::PartiallyApplied), error.effect_state());
        assert!(std::error::Error::source(&error).is_some());
    }
}
