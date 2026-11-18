//! Single Responsibility (SRP/SOLID): Pure domain value objects.

pub mod sanitized_name;

pub use sanitized_name::{SanitizedName, sanitize_filename};
