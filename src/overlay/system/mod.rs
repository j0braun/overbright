//! The overlay's system readers: `/proc` and `/sys` sources, and NVML.

#[cfg(target_os = "linux")]
pub mod nvml;
pub mod sources;
