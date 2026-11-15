use serde::{Deserialize, Serialize};

/// RFC-032: `Serialize`/`Deserialize` and `Default` (terminal-resize
/// handoff's sibling security decision) added so this can be persisted
/// directly in `RecentProject::trust_state` -- see that field's own doc
/// comment for why a real typed value replaced a display-only string.
/// `Default` is `Restricted`, the fail-closed choice: an on-disk record
/// from before this field existed omits it, and `#[serde(default)]`
/// there falls back to this -- correct as more than a parser
/// convenience, since nothing could have been trusted before this RFC
/// gave `grant_project_trust` its first production caller.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum WorkspaceTrust {
    Unknown,
    #[default]
    Restricted,
    Trusted,
    Revoked,
}

impl WorkspaceTrust {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Restricted => "Restricted",
            Self::Trusted => "Trusted",
            Self::Revoked => "Revoked",
        }
    }
}
