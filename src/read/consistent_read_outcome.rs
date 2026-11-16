use crate::metadata::ResourceVersion;
/// Result metadata for a read with an explicit consistency policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsistentReadOutcome<T> {
    /// Read value.
    pub value: T,
    /// Version token observed on the opened object, when available.
    pub version: Option<ResourceVersion>,
    /// Whether the provider observed a change during the read.
    pub changed: Option<bool>,
}
