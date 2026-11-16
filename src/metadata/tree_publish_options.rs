use super::{AtomicityRequirement, DurabilityRequirement};
use super::{FileSystemCapabilities, FileSystemCapability, FileSystemCapabilitySupport};
use crate::error::{FsError, FsErrorKind, FsOperation, FsResult};
/// Publication guarantees requested for a staged directory tree.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TreePublishOptions {
    /// Required namespace publication atomicity.
    pub atomicity: AtomicityRequirement,
    /// Required persistence strength after publication.
    pub durability: DurabilityRequirement,
}

impl TreePublishOptions {
    /// Rejects required guarantees known to be unsupported before publication
    /// begins. Conditional capabilities remain eligible for provider-specific
    /// preflight against the concrete staging request.
    pub fn validate_capabilities(&self, capabilities: &FileSystemCapabilities) -> FsResult<()> {
        let required = match self.atomicity {
            AtomicityRequirement::Required => Some(FileSystemCapability::AtomicTreeCopy),
            AtomicityRequirement::Preferred | AtomicityRequirement::NotRequired => None,
        };
        if let Some(capability) = required {
            if capabilities.support(capability) == FileSystemCapabilitySupport::Unsupported {
                return Err(missing_tree_capability(capability));
            }
        }
        let required = match self.durability {
            DurabilityRequirement::Required => Some(FileSystemCapability::DurableTreeCopy),
            DurabilityRequirement::Preferred | DurabilityRequirement::NotRequired => None,
        };
        if let Some(capability) = required {
            if capabilities.support(capability) == FileSystemCapabilitySupport::Unsupported {
                return Err(missing_tree_capability(capability));
            }
        }
        Ok(())
    }
}

fn missing_tree_capability(capability: FileSystemCapability) -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedCapability,
        FsOperation::ValidateProviderOutcome,
        "required tree publication guarantee is unsupported",
    )
    .with_required_capability(capability)
}
