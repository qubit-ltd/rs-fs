/// Content-based precondition for a mutation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ContentPrecondition {
    /// Do not compare existing content.
    #[default]
    None,
    /// Require current content to equal the supplied bytes.
    Equals(Vec<u8>),
}
