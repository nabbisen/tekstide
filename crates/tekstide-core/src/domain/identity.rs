use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::path::PathBuf;

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

/// RFC-077 D7.1 (review 696 §3, corrected by §10 / scoping §10): "which
/// project, and where its root is" -- the two values `change_detection`
/// reads from a `&ProjectSession` and nothing else (measured: `id()` at
/// 7 call sites, `canonical_root_path()` at 11). Moved here so that
/// module no longer needs to name `ProjectSession` across the
/// `change_detection -> session` back-edge RFC-077 D7 is blocked on.
///
/// **Deliberately not named anything containing "Valid."** Unlike
/// `project::root::ValidProjectRoot`, this carries no validation
/// guarantee of its own -- it is plain identity-plus-location
/// vocabulary, so its fields are public and it has no restricted
/// constructor. `ValidProjectRoot` stays defined in `project::root`,
/// the only place that can enforce the guarantee it claims (E0742 --
/// see `ARCHITECTURE.md`'s seventh invariant, the same reason
/// `approval -> agent` is permanent).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRootLocation {
    pub id: ProjectId,
    pub canonical_root_path: PathBuf,
}

impl ProjectRootLocation {
    /// Matches `ProjectSession::id`'s own shape, so every existing
    /// `change_detection` call site keeps reading `project.id()` --
    /// only the parameter's type changed, not the body.
    pub fn id(&self) -> &ProjectId {
        &self.id
    }

    /// Matches `ProjectSession::canonical_root_path`'s own shape
    /// exactly, including the return type (`&PathBuf`, not `&Path`) --
    /// `change_detection`'s own comparisons (`*project.canonical_root_path()
    /// == canonical_path`) rely on it being the same type on both
    /// sides, so this is not merely cosmetic.
    pub fn canonical_root_path(&self) -> &PathBuf {
        &self.canonical_root_path
    }
}
