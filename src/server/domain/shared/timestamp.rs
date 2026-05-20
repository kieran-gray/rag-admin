use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp(String);

impl Timestamp {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for Timestamp {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Timestamp {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_as_str_roundtrip() {
        let ts = Timestamp::new("2024-05-01T12:30:00Z".to_string());
        assert_eq!(ts.as_str(), "2024-05-01T12:30:00Z");
    }

    #[test]
    fn from_string_and_str_produce_equal_values() {
        let a: Timestamp = String::from("2024-01-01T00:00:00Z").into();
        let b: Timestamp = "2024-01-01T00:00:00Z".into();
        assert_eq!(a, b);
    }

    #[test]
    fn display_renders_inner_string() {
        let ts = Timestamp::from("now");
        assert_eq!(format!("{ts}"), "now");
    }

    #[test]
    fn serde_roundtrip_preserves_value() {
        let ts = Timestamp::from("2024-05-01T12:30:00Z");
        let serialized = serde_json::to_string(&ts).expect("serialize");
        let restored: Timestamp = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(ts, restored);
    }
}
