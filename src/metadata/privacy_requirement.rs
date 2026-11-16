/// Privacy strength requested for a temporary filesystem resource.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PrivacyRequirement {
    /// Use provider defaults.
    #[default]
    Preferred,
    /// Restrict access to the creating principal.
    Private,
}
