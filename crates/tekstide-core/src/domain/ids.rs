use std::fmt;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TerminalId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AgentRunId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ApprovalId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TranscriptId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChangeSetId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuditEventId(String);

macro_rules! impl_id {
    ($type_name:ident, $prefix:literal) => {
        impl $type_name {
            pub fn new_uuid() -> Self {
                Self(format!("{}-{}", $prefix, uuid::Uuid::new_v4()))
            }

            #[cfg(test)]
            pub fn for_test(sequence: u64) -> Self {
                Self(format!("{}-{:012x}", $prefix, sequence))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $type_name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

impl_id!(TerminalId, "terminal");
impl_id!(AgentRunId, "agent-run");
impl_id!(ApprovalId, "approval");
impl_id!(TranscriptId, "transcript");
impl_id!(ChangeSetId, "changeset");
impl_id!(AuditEventId, "audit");
