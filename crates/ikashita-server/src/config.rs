//! Local server configuration.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::StaticBundle;

/// Defaults for serving a validated application locally.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    address: SocketAddr,
    bundle: Option<StaticBundle>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::localhost()
    }
}

impl ServerConfig {
    /// Creates the MVP localhost default at `127.0.0.1:8787`.
    #[must_use]
    pub const fn localhost() -> Self {
        Self { address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8787), bundle: None }
    }

    /// Replaces the listen address.
    #[must_use]
    pub const fn with_address(mut self, address: SocketAddr) -> Self {
        self.address = address;
        self
    }

    /// Attaches the static bundle to be served.
    #[must_use]
    pub fn with_bundle(mut self, bundle: StaticBundle) -> Self {
        self.bundle = Some(bundle);
        self
    }

    /// Returns the configured listen address.
    #[must_use]
    pub const fn address(&self) -> SocketAddr {
        self.address
    }

    /// Returns the configured static bundle, when one has been attached.
    #[must_use]
    pub const fn bundle(&self) -> Option<&StaticBundle> {
        self.bundle.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_localhost() {
        assert_eq!(ServerConfig::default().address(), "127.0.0.1:8787".parse().unwrap());
        assert!(ServerConfig::default().bundle().is_none());
    }
}
