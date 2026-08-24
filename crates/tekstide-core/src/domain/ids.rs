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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuditOperationId(String);

macro_rules! impl_id {
    ($type_name:ident, $prefix:literal) => {
        impl $type_name {
            pub fn new_uuid() -> Self {
                Self(format!("{}-{}", $prefix, uuid::Uuid::new_v4()))
            }

            /// RFC-039 PR-039-C: a deterministic id whose suffix is still
            /// a real UUID, not the plain hex sequence this used to
            /// produce -- that shape passed [`Self::from_persisted`]'s
            /// own doc-implied contract in every existing caller only
            /// because none of them had gone through a real
            /// `AuditStore` round trip before (every prior use was
            /// either a bare `.validate()` check with no SQL involved,
            /// or an `operation_id`-less record). `for_test(1)` and
            /// `for_test(2)` are still guaranteed distinct and stable
            /// across runs -- `Uuid::from_u128` is a pure function of
            /// its input, not random -- so no caller's own assertions
            /// change; only decodability does.
            #[cfg(test)]
            pub fn for_test(sequence: u64) -> Self {
                Self(format!(
                    "{}-{}",
                    $prefix,
                    uuid::Uuid::from_u128(u128::from(sequence))
                ))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn from_persisted(value: impl Into<String>) -> Option<Self> {
                let value = value.into();
                let suffix = value.strip_prefix(concat!($prefix, "-"))?;
                uuid::Uuid::parse_str(suffix).ok()?;
                Some(Self(value))
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
impl_id!(AuditOperationId, "audit-operation");
