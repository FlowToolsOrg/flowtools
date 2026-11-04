#[cfg(not(all(windows, target_arch = "x86_64")))]
compile_error!("Managed real-media candidate validation requires Windows/x64; an empty suite is not acceptance.");

// These are the actual source modules, including their private cfg(test) hooks.
// There is no fork, test-authority feature on the product, or dependency from it.
include!("../../../../../packages/runtime-core/src/lib.rs");

#[cfg(test)]
mod managed_media;
