/// Resource limits used while validating a staged directory tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TreeValidationOptions {
    /// Maximum number of entries.
    pub max_entries: usize,
    /// Maximum relative path depth.
    pub max_depth: usize,
    /// Maximum aggregate file content bytes.
    pub max_content_bytes: u64,
}
impl Default for TreeValidationOptions {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_depth: 128,
            max_content_bytes: 1 << 40,
        }
    }
}
