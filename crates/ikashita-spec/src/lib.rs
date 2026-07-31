//! Versioned metadata for the ikashita application definition.
//!
//! Parsing and validation will consume this profile metadata. Keeping the
//! profile identity in its own crate lets CLI, server, and host adapters agree
//! on compatibility without depending on a particular parser or renderer.

pub mod profile;

pub use profile::{
    ApplicationProfile, KDL_APPLICATION_PROFILE, MVP_PROFILE_VERSION, ProfileVersion,
};
