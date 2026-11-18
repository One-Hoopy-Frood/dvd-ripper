//! Single Responsibility (SRP/SOLID): Pure domain value object and logic for filesystem-safe naming.

/// Sanitizes a string to make it safe for filesystem folders and file names across all operating systems.
///
/// Strips illegal characters (`:`, `\`, `/`, `*`, `?`, `"`, `<`, `>`, `|`), control characters,
/// and collapses excessive whitespace.
pub fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            ':' => ' ',
            '\\' | '/' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => ' ',
            _ => c,
        })
        .collect();

    let mut result = String::with_capacity(sanitized.len());
    for word in sanitized.split_whitespace() {
        if !result.is_empty() {
            result.push(' ');
        }
        result.push_str(word);
    }
    result
}

/// An immutable, validated sanitized string suitable for file and directory names.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SanitizedName(String);

impl SanitizedName {
    pub fn new(raw: &str) -> Self {
        Self(sanitize_filename(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for SanitizedName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SanitizedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            sanitize_filename("The Matrix: Reloaded"),
            "The Matrix Reloaded"
        );
        assert_eq!(
            sanitize_filename("A/B\\C*D?E\"F<G>H|I"),
            "A_B_C_D_E_F_G_H_I"
        );
        assert_eq!(sanitize_filename("  Extra   Spaces  "), "Extra Spaces");
    }

    #[test]
    fn test_sanitized_name_value_object() {
        let name = SanitizedName::new("Movie: Special Edition");
        assert_eq!(name.as_str(), "Movie Special Edition");
        assert_eq!(name.to_string(), "Movie Special Edition");
    }
}
