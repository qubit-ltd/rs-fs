/// Consistency strength requested for a read operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReadConsistencyPolicy {
    /// Read the opened object without additional change detection.
    #[default]
    BestEffort,
    /// Observe changes when the provider can detect them.
    ObserveChanges,
    /// Require a version token bound to the same opened object.
    RequireVersion,
}
