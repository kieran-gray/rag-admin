use std::fmt;

use serde::de::{self, Visitor};
use serde::Deserializer;

pub(crate) fn u32_from_string<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(U32Visitor)
}

struct U32Visitor;

impl Visitor<'_> for U32Visitor {
    type Value = u32;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a u32 number or decimal string")
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        u32::try_from(value).map_err(E::custom)
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value < 0 {
            return Err(E::custom("u32 cannot be negative"));
        }
        u32::try_from(value as u64).map_err(E::custom)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.trim().parse::<u32>().map_err(E::custom)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(&value)
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Example {
        #[serde(deserialize_with = "super::u32_from_string")]
        value: u32,
    }

    #[test]
    fn accepts_number() {
        let parsed: Example = serde_json::from_str(r#"{"value":1024}"#).unwrap();
        assert_eq!(parsed.value, 1024);
    }

    #[test]
    fn accepts_string() {
        let parsed: Example = serde_json::from_str(r#"{"value":"1024"}"#).unwrap();
        assert_eq!(parsed.value, 1024);
    }
}
