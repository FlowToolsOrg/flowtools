//! Prototype Windows/x64 feasibility harness, never a production plugin API.
//! This crate has no dependency on the Runtime, SDK, installed inventory or grants.
#[cfg(all(windows, target_arch = "x86_64"))]
mod windows;
#[cfg(all(windows, target_arch = "x86_64"))]
pub use windows::{Fixture, Outcome, Rejection, TokenEvidence};

pub const INPUT_LIMIT: usize = 8 * 1024;
pub const OUTPUT_LIMIT: usize = 16 * 1024;
pub const WALL_LIMIT_MS: u64 = 750;
pub const TERMINATE_GRACE_MS: u64 = 500;

#[cfg(test)]
mod tests {
    #[test]
    fn profile_is_bounded_and_has_no_production_dependency() {
        const { assert!(super::INPUT_LIMIT < super::OUTPUT_LIMIT) };
        assert_eq!(super::TERMINATE_GRACE_MS, 500);
        let manifest = include_str!("../Cargo.toml");
        assert!(!manifest.contains("flowtools-runtime"));
        assert!(!manifest.contains("flowtools-sdk"));
    }
}
