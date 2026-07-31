//! KDL Application Profile identity and compatibility values.

use std::fmt;

/// The stable identifier of the application profile used by the MVP.
pub const KDL_APPLICATION_PROFILE: &str = "kdl.application";

/// The first executable profile version.
pub const MVP_PROFILE_VERSION: ProfileVersion = ProfileVersion { major: 0, minor: 1 };

/// A major/minor application profile version.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProfileVersion {
    /// Major version; incompatible profile changes increment this value.
    pub major: u16,
    /// Minor version; compatible profile additions increment this value.
    pub minor: u16,
}

impl fmt::Display for ProfileVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)
    }
}

/// The versioned identity of one validated application definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationProfile {
    /// Application name from the profile's `app` node.
    pub name: String,
    /// Profile version used to interpret the definition.
    pub version: ProfileVersion,
}

impl ApplicationProfile {
    /// Creates an application using KDL Application Profile v0.1.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), version: MVP_PROFILE_VERSION }
    }

    /// Returns the profile identifier paired with this application.
    #[must_use]
    pub const fn profile_id(&self) -> &'static str {
        KDL_APPLICATION_PROFILE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_application_uses_the_mvp_profile() {
        let profile = ApplicationProfile::new("contacts-admin");

        assert_eq!(profile.profile_id(), KDL_APPLICATION_PROFILE);
        assert_eq!(profile.version, MVP_PROFILE_VERSION);
        assert_eq!(profile.version.to_string(), "0.1");
    }
}
