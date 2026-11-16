//! Shader composition (audit plan render §0 (a)): the composer, the
//! registry of modules by typed key, and the validation tests.

pub mod compose;
pub mod registry;

#[cfg(test)]
mod validate_tests;
