//! Explicit directory and object-key listing filters.
/// Listing selection mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListFilter {
    /// Select a path and its descendants using component boundaries.
    Subtree(String),
    /// Select keys by their raw string prefix.
    LiteralPrefix(String),
}
