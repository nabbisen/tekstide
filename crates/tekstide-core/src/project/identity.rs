use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectId(String);

impl ProjectId {
    pub fn new_uuid() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_persisted(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        uuid::Uuid::parse_str(&value).ok()?;
        Some(Self(value))
    }

    #[cfg(test)]
    pub fn for_test(sequence: u64) -> Self {
        Self(format!("00000000-0000-4000-8000-{sequence:012x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for ProjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_persisted(value)
            .ok_or_else(|| serde::de::Error::custom("project_id must be a UUID string"))
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
