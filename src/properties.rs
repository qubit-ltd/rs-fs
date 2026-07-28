//! Immutable filesystem property snapshots used by facades.

use crate::{
    FileSystemCapabilities, FileSystemInfo, FileSystemLimits, FsError, FsErrorKind, FsOperation,
    FsResult, Path, PathSemantics,
};

/// Permitted absolute or relative form for paths accepted by a filesystem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathForm {
    /// Only absolute paths are accepted.
    Absolute,
    /// Only relative paths are accepted.
    Relative,
    /// Both absolute and relative paths are accepted.
    Either,
}

/// Immutable path form constraints attached to one filesystem snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathConstraints {
    form: PathForm,
}

impl PathConstraints {
    /// Creates constraints accepting only absolute paths.
    #[must_use]
    pub const fn absolute() -> Self {
        Self {
            form: PathForm::Absolute,
        }
    }

    /// Creates constraints accepting only relative paths.
    #[must_use]
    pub const fn relative() -> Self {
        Self {
            form: PathForm::Relative,
        }
    }

    /// Creates constraints accepting either logical path form.
    #[must_use]
    pub const fn either() -> Self {
        Self {
            form: PathForm::Either,
        }
    }

    /// Returns the configured accepted path form.
    #[must_use]
    pub const fn form(&self) -> PathForm {
        self.form
    }

    /// Validates a logical path without performing I/O.
    ///
    /// Returns an invalid-path error when `path` has a disallowed form.
    pub fn validate(&self, path: &Path) -> FsResult<()> {
        let allowed = matches!(self.form, PathForm::Either)
            || matches!(
                (self.form, path.is_absolute()),
                (PathForm::Absolute, true) | (PathForm::Relative, false)
            );
        if allowed {
            Ok(())
        } else {
            Err(FsError::invalid_path(
                FsOperation::ParsePath,
                "path form is not accepted by this filesystem",
            ))
        }
    }
}

/// Immutable construction-time properties cached by a filesystem facade.
#[derive(Clone, Debug)]
pub struct FileSystemProperties {
    /// Stable filesystem information.
    info: FileSystemInfo,
    /// Stable advertised capabilities.
    capabilities: FileSystemCapabilities,
    /// Stable provider limits.
    limits: FileSystemLimits,
    /// Accepted logical path forms.
    path_constraints: PathConstraints,
}

impl FileSystemProperties {
    /// Builds and validates an immutable filesystem property snapshot.
    ///
    /// Returns an invalid-options error when the provider identity is invalid,
    /// advertised capabilities violate dependencies, or path configuration is
    /// internally inconsistent. This method performs no I/O.
    pub fn new(
        info: FileSystemInfo,
        capabilities: FileSystemCapabilities,
        limits: FileSystemLimits,
        path_constraints: PathConstraints,
    ) -> FsResult<Self> {
        let properties = Self {
            info,
            capabilities,
            limits,
            path_constraints,
        };
        properties.validate()?;
        Ok(properties)
    }

    /// Returns the stable filesystem identity and configuration.
    #[must_use]
    pub const fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    /// Returns the stable advertised capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> FileSystemCapabilities {
        self.capabilities
    }

    /// Returns the stable filesystem limits.
    #[must_use]
    pub const fn limits(&self) -> &FileSystemLimits {
        &self.limits
    }

    /// Returns the immutable accepted path constraints.
    #[must_use]
    pub const fn path_constraints(&self) -> &PathConstraints {
        &self.path_constraints
    }

    /// Defensively validates a provider-supplied snapshot at the facade boundary.
    ///
    /// Returns an invalid-options error when the snapshot violates core value
    /// invariants. It performs no I/O and is intentionally crate-private.
    pub(crate) fn validate(&self) -> FsResult<()> {
        if self.info.provider_id().is_empty()
            || self.info.provider_id().chars().any(char::is_control)
        {
            return Err(invalid_properties(
                "provider id must be non-empty and contain no controls",
            ));
        }
        if let Some((_capability, _dependency)) = self.capabilities.missing_dependency() {
            return Err(invalid_properties(
                "advertised capability dependency is missing",
            ));
        }
        if [
            self.limits.max_path_text_bytes(),
            self.limits.max_component_text_bytes(),
            self.limits.max_read_range_bytes(),
            self.limits.max_write_bytes(),
            self.limits.max_list_page_entries(),
        ]
        .into_iter()
        .any(|limit| matches!(limit, crate::FileSystemLimit::Maximum(0)))
        {
            return Err(invalid_properties(
                "finite filesystem limits must have a positive value",
            ));
        }
        if self.info.path_semantics() != PathSemantics::Hierarchical
            && self.path_constraints.form() == PathForm::Absolute
        {
            return Err(invalid_properties(
                "literal path semantics cannot require hierarchical absolute paths",
            ));
        }
        Ok(())
    }
}

/// Builds the shared property-validation failure.
fn invalid_properties(message: &'static str) -> FsError {
    FsError::new(
        FsErrorKind::InvalidOptions,
        FsOperation::ValidateProperties,
        message,
    )
}
