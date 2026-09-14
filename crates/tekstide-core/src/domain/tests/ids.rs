use crate::domain::AgentRunId;

/// RFC-050 PR-050-A: `AgentRunId::from_persisted` rejects a string that is
/// not a UUID, and one without the `agent-run-` prefix every run directory
/// carries. The constructor already existed — `impl_id!` gives every id type
/// `from_persisted` — although the RFC measured it as missing.
///
/// **This does not prove "only what this product writes."** `uuid::Uuid::parse_str`
/// also accepts uppercase, hyphen-less, braced and `urn:uuid:` spellings, and
/// `from_persisted` inherits that — measured by a throwaway probe for request
/// 388, for `ProjectId` too. The product only ever writes lowercase hyphenated
/// ids, so PR-050-B's loader would accept directory names this product never
/// wrote. Tightening this changes what persisted state parses, which is a
/// decision for the architect, not a rider on this slice.
#[test]
fn agent_run_id_rejects_non_uuid_and_unprefixed_names() {
    let written = AgentRunId::new_uuid();
    assert_eq!(
        AgentRunId::from_persisted(written.as_str()),
        Some(written.clone())
    );

    let bare_uuid = &written.as_str()["agent-run-".len()..];
    for rejected in [
        "not-a-uuid",
        "agent-run-not-a-uuid",
        "agent-run-",
        // A bare UUID names a *project* directory, never a run's.
        bare_uuid,
        "../agent-run-00000000-0000-4000-8000-000000000001",
        "agent-run-00000000-0000-4000-8000-000000000001/..",
    ] {
        assert_eq!(
            AgentRunId::from_persisted(rejected),
            None,
            "{rejected:?} is not a name this product writes"
        );
    }
}
