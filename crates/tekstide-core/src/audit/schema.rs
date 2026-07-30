pub const AUDIT_APPLICATION_ID: i64 = 0x544B_4155;
pub const AUDIT_SCHEMA_VERSION: i64 = 2;

/// The pre-Amendment-1 v1 `audit_events` DDL, byte-for-byte. **Never
/// edit this to match v2** -- it is the definition of *version one*, and
/// its only value is that it now disagrees with the current schema. It
/// must match `audit/tests/fixtures/audit-v1.sql` exactly (a dedicated
/// test asserts this); that fixture is immutable for the same reason.
///
/// This is the constant `3ac794b` should have left alone and instead
/// added a v2 sibling next to -- see [`CREATE_SCHEMA_V2`] and
/// `audit::migration`'s `1 -> 2` `MigrationStep` for how that is done
/// correctly (RFC-013 Amendment 1).
///
/// No production code executes this any more -- a real installation
/// reaches v2 either fresh ([`CREATE_SCHEMA_V2`]) or via the `1 -> 2`
/// migration, never by running v1 DDL directly. Kept `pub` and
/// `#[allow(dead_code)]` rather than deleted: it is exercised by
/// `create_schema_v1_constant_matches_the_immutable_fixture_exactly`
/// (the regression guard against exactly this constant drifting again),
/// and is the historical record of what a real prior installation's
/// schema was.
#[allow(dead_code)]
pub const CREATE_SCHEMA_V1: &str = r#"
CREATE TABLE audit_events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    project_id TEXT,
    family TEXT NOT NULL CHECK (family IN (
        'project_added', 'trust_change', 'command_approval',
        'managed_process_lifecycle', 'plain_terminal_observation',
        'paste_blocked', 'restricted_mode_blocked', 'root_access_blocked',
        'safe_close_decision', 'sensitive_config_changed',
        'transcript_purge', 'audit_store_recovery'
    )),
    outcome TEXT NOT NULL CHECK (outcome IN (
        'requested', 'authorized', 'applied', 'failed', 'started',
        'terminated', 'blocked', 'cancelled', 'completed'
    )),
    operation_id TEXT,
    terminal_id TEXT,
    agent_run_id TEXT,
    approval_id TEXT,
    subject_kind TEXT CHECK (subject_kind IS NULL OR subject_kind IN (
        'app_resource', 'transcript', 'recovery_bundle'
    )),
    subject_ref TEXT CHECK (subject_ref IS NULL OR length(subject_ref) BETWEEN 1 AND 128),
    action_kind TEXT NOT NULL CHECK (action_kind IN (
        'project_add', 'trust_grant', 'trust_revoke', 'command_request',
        'command_approve', 'command_edit_and_approve', 'command_reject',
        'managed_agent_launch', 'plain_terminal_lifecycle', 'terminal_paste',
        'restricted_feature', 'root_access', 'safe_close_terminate',
        'safe_close_abandon', 'destructive_action', 'config_policy_increase',
        'config_policy_reduce', 'transcript_purge', 'audit_store_recovery'
    )),
    risk_level TEXT CHECK (risk_level IS NULL OR risk_level IN (
        'low', 'medium', 'high', 'destructive'
    )),
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'app_policy', 'runtime')),
    action_source TEXT NOT NULL CHECK (action_source IN (
        'trusted_ui', 'app_command', 'policy_engine', 'adapter',
        'runtime_observer', 'explicit_cleanup'
    )),
    adapter_profile_ref TEXT CHECK (
        adapter_profile_ref IS NULL OR length(adapter_profile_ref) BETWEEN 1 AND 128
    ),
    reason_code TEXT CHECK (reason_code IS NULL OR reason_code IN (
        'root_escape', 'symlink_escape', 'restricted_mode', 'paste_policy',
        'user_cancelled', 'runtime_failure', 'storage_failure', 'process_exited',
        'process_terminated', 'policy_changed', 'recovery_completed'
    )),
    created_at TEXT NOT NULL,
    CHECK ((subject_kind IS NULL) = (subject_ref IS NULL)),
    CHECK (subject_ref IS NULL OR (
        length(subject_ref) BETWEEN 1 AND 128
        AND subject_ref NOT GLOB '*[^A-Za-z0-9_.:-]*'
    )),
    CHECK (adapter_profile_ref IS NULL OR (
        length(adapter_profile_ref) BETWEEN 1 AND 128
        AND adapter_profile_ref NOT GLOB '*[^A-Za-z0-9_.:-]*'
    )),
    CHECK (
        (actor_kind = 'user' AND action_source IN ('trusted_ui', 'app_command'))
        OR (actor_kind = 'app_policy' AND action_source IN (
            'policy_engine', 'adapter', 'explicit_cleanup'
        ))
        OR (actor_kind = 'runtime' AND action_source = 'runtime_observer')
    ),
    CHECK (
        (family = 'project_added'
            AND project_id IS NOT NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL AND reason_code IS NULL
            AND action_kind = 'project_add' AND outcome = 'applied'
            AND ((actor_kind = 'user' AND action_source IN ('trusted_ui', 'app_command'))
                OR (actor_kind = 'app_policy' AND action_source = 'policy_engine')))
        OR (family = 'trust_change'
            AND project_id IS NOT NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL AND reason_code IS NULL
            AND ((actor_kind = 'user' AND action_source = 'trusted_ui')
                OR (actor_kind = 'app_policy' AND action_source = 'policy_engine'))
            AND ((action_kind = 'trust_grant' AND operation_id IS NOT NULL
                    AND outcome IN ('authorized', 'applied', 'failed'))
                OR (action_kind = 'trust_revoke' AND operation_id IS NULL
                    AND outcome = 'applied')))
        OR (family = 'command_approval'
            AND project_id IS NOT NULL AND approval_id IS NOT NULL
            AND terminal_id IS NULL AND subject_kind IS NULL
            AND risk_level IS NOT NULL AND reason_code IS NULL
            AND ((action_kind = 'command_request' AND operation_id IS NULL
                    AND outcome = 'requested' AND actor_kind = 'app_policy'
                    AND action_source = 'adapter')
                OR (action_kind IN ('command_approve', 'command_edit_and_approve')
                    AND operation_id IS NOT NULL
                    AND outcome IN ('authorized', 'applied', 'failed')
                    AND actor_kind = 'user' AND action_source = 'trusted_ui')
                OR (action_kind = 'command_reject' AND operation_id IS NULL
                    AND outcome = 'applied' AND actor_kind = 'user'
                    AND action_source = 'trusted_ui')))
        OR (family = 'managed_process_lifecycle'
            AND project_id IS NOT NULL AND agent_run_id IS NOT NULL
            AND approval_id IS NULL AND operation_id IS NOT NULL
            AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NOT NULL
            AND action_kind = 'managed_agent_launch'
            AND ((outcome = 'authorized' AND reason_code IS NULL
                    AND ((actor_kind = 'user'
                            AND action_source IN ('trusted_ui', 'app_command'))
                        OR (actor_kind = 'app_policy' AND action_source = 'policy_engine')))
                OR (outcome = 'started' AND terminal_id IS NOT NULL
                    AND reason_code IS NULL AND actor_kind = 'runtime'
                    AND action_source = 'runtime_observer')
                OR (outcome = 'failed' AND reason_code IS NOT NULL
                    AND actor_kind = 'runtime' AND action_source = 'runtime_observer')
                OR (outcome = 'terminated' AND terminal_id IS NOT NULL
                    AND reason_code IS NOT NULL AND actor_kind = 'runtime'
                    AND action_source = 'runtime_observer')))
        OR (family = 'plain_terminal_observation'
            AND project_id IS NOT NULL AND terminal_id IS NOT NULL
            AND agent_run_id IS NULL AND approval_id IS NULL AND operation_id IS NULL
            AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL
            AND action_kind = 'plain_terminal_lifecycle'
            AND actor_kind = 'runtime' AND action_source = 'runtime_observer'
            AND outcome IN ('started', 'failed', 'terminated')
            AND (outcome = 'started' OR reason_code IS NOT NULL))
        OR (family = 'paste_blocked'
            AND project_id IS NOT NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL AND reason_code = 'paste_policy'
            AND action_kind = 'terminal_paste' AND outcome = 'blocked'
            AND actor_kind = 'app_policy' AND action_source = 'policy_engine')
        OR (family = 'restricted_mode_blocked'
            AND project_id IS NOT NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL AND reason_code = 'restricted_mode'
            AND action_kind = 'restricted_feature' AND outcome = 'blocked'
            AND actor_kind = 'app_policy' AND action_source = 'policy_engine')
        OR (family = 'root_access_blocked'
            AND project_id IS NOT NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL
            AND reason_code IN ('root_escape', 'symlink_escape')
            AND action_kind = 'root_access' AND outcome = 'blocked'
            AND actor_kind = 'app_policy' AND action_source = 'policy_engine')
        OR (family = 'safe_close_decision'
            AND project_id IS NOT NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND (subject_kind IS NULL OR subject_kind = 'app_resource')
            AND risk_level IS NULL AND adapter_profile_ref IS NULL
            AND action_kind IN ('safe_close_terminate', 'safe_close_abandon', 'destructive_action')
            AND actor_kind = 'user' AND action_source IN ('trusted_ui', 'app_command')
            AND ((outcome IN ('authorized', 'applied', 'failed')
                    AND operation_id IS NOT NULL)
                OR (outcome = 'cancelled' AND operation_id IS NULL)))
        OR (family = 'sensitive_config_changed'
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND subject_kind IS NULL AND risk_level IS NULL AND adapter_profile_ref IS NULL
            AND reason_code = 'policy_changed'
            AND ((actor_kind = 'user' AND action_source = 'trusted_ui')
                OR (actor_kind = 'app_policy' AND action_source = 'policy_engine'))
            AND ((action_kind = 'config_policy_increase' AND operation_id IS NOT NULL
                    AND outcome IN ('authorized', 'applied', 'failed'))
                OR (action_kind = 'config_policy_reduce' AND operation_id IS NULL
                    AND outcome = 'applied')))
        OR (family = 'transcript_purge'
            AND project_id IS NOT NULL AND terminal_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind = 'transcript'
            AND risk_level IS NULL AND adapter_profile_ref IS NULL AND reason_code IS NULL
            AND action_kind = 'transcript_purge' AND outcome IN ('completed', 'failed')
            AND ((actor_kind = 'user' AND action_source IN ('trusted_ui', 'app_command'))
                OR (actor_kind = 'app_policy' AND action_source = 'explicit_cleanup')))
        OR (family = 'audit_store_recovery'
            AND project_id IS NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind = 'recovery_bundle'
            AND risk_level IS NULL AND adapter_profile_ref IS NULL
            AND reason_code = 'recovery_completed'
            AND action_kind = 'audit_store_recovery' AND outcome = 'completed'
            AND actor_kind = 'user' AND action_source IN ('trusted_ui', 'app_command'))
    )
);

CREATE INDEX audit_events_project_sequence
    ON audit_events(project_id, sequence DESC);
CREATE INDEX audit_events_operation_sequence
    ON audit_events(operation_id, sequence ASC);
CREATE UNIQUE INDEX audit_events_one_authorization_per_operation
    ON audit_events(operation_id) WHERE outcome = 'authorized';
CREATE INDEX audit_events_family_outcome_sequence
    ON audit_events(family, outcome, sequence DESC);
"#;

/// The `audit_events` table body (every column and constraint, v2 shape),
/// parametrized only by table name.
///
/// RFC-013 Amendment 1's own lesson, applied mechanically rather than by
/// discipline: commit `3ac794b` edited `CREATE_SCHEMA_V1`'s `CHECK`
/// constraints in place, which is exactly the "two copies of the same DDL
/// drift apart" failure a hand-maintained second constant would repeat.
/// This macro's body exists as literal text **exactly once** in the
/// source; every expansion site (a fresh v2 install's [`CREATE_SCHEMA_V2`]
/// and `audit::migration`'s `1 -> 2` table-rebuild statement) is stamped
/// from this one definition, so they cannot diverge from each other --
/// only the table name differs, and SQLite's `ALTER TABLE ... RENAME TO`
/// rewrites the stored name on rename, so a migrated table's
/// `sqlite_master.sql` ends up identical to a fresh install's. The
/// convergence test in `audit::tests::migration` verifies this holds in
/// practice, not just in theory -- which is how the identifier here was
/// found to need explicit double-quoting in the first place: SQLite's
/// rename rewrites the stored name to `"audit_events"` (quoted), not
/// `audit_events` (bare), so a fresh install must be written pre-quoted
/// too, or the two paths' `sqlite_master.sql` disagree on quoting alone
/// despite describing the identical table.
macro_rules! audit_events_v2_table_ddl {
    ($table_name:literal) => {
        concat!(
            "CREATE TABLE \"",
            $table_name,
            "\" (",
            r#"
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    project_id TEXT,
    family TEXT NOT NULL CHECK (family IN (
        'project_added', 'trust_change', 'command_approval',
        'managed_process_lifecycle', 'plain_terminal_observation',
        'paste_blocked', 'restricted_mode_blocked', 'root_access_blocked',
        'safe_close_decision', 'sensitive_config_changed',
        'transcript_purge', 'audit_store_recovery'
    )),
    outcome TEXT NOT NULL CHECK (outcome IN (
        'requested', 'authorized', 'applied', 'failed', 'started',
        'terminated', 'blocked', 'cancelled', 'completed', 'anomaly'
    )),
    operation_id TEXT,
    terminal_id TEXT,
    agent_run_id TEXT,
    approval_id TEXT,
    subject_kind TEXT CHECK (subject_kind IS NULL OR subject_kind IN (
        'app_resource', 'transcript', 'recovery_bundle'
    )),
    subject_ref TEXT CHECK (subject_ref IS NULL OR length(subject_ref) BETWEEN 1 AND 128),
    action_kind TEXT NOT NULL CHECK (action_kind IN (
        'project_add', 'trust_grant', 'trust_revoke', 'command_request',
        'command_approve', 'command_edit_and_approve', 'command_reject',
        'command_cwd_mismatch',
        'managed_agent_launch', 'plain_terminal_lifecycle', 'terminal_paste',
        'restricted_feature', 'root_access', 'safe_close_terminate',
        'safe_close_abandon', 'destructive_action', 'config_policy_increase',
        'config_policy_reduce', 'transcript_purge', 'audit_store_recovery'
    )),
    risk_level TEXT CHECK (risk_level IS NULL OR risk_level IN (
        'low', 'medium', 'high', 'destructive'
    )),
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'app_policy', 'runtime')),
    action_source TEXT NOT NULL CHECK (action_source IN (
        'trusted_ui', 'app_command', 'policy_engine', 'adapter',
        'runtime_observer', 'explicit_cleanup'
    )),
    adapter_profile_ref TEXT CHECK (
        adapter_profile_ref IS NULL OR length(adapter_profile_ref) BETWEEN 1 AND 128
    ),
    reason_code TEXT CHECK (reason_code IS NULL OR reason_code IN (
        'root_escape', 'symlink_escape', 'restricted_mode', 'paste_policy',
        'user_cancelled', 'runtime_failure', 'storage_failure', 'process_exited',
        'process_terminated', 'policy_changed', 'recovery_completed'
    )),
    created_at TEXT NOT NULL,
    CHECK ((subject_kind IS NULL) = (subject_ref IS NULL)),
    CHECK (subject_ref IS NULL OR (
        length(subject_ref) BETWEEN 1 AND 128
        AND subject_ref NOT GLOB '*[^A-Za-z0-9_.:-]*'
    )),
    CHECK (adapter_profile_ref IS NULL OR (
        length(adapter_profile_ref) BETWEEN 1 AND 128
        AND adapter_profile_ref NOT GLOB '*[^A-Za-z0-9_.:-]*'
    )),
    CHECK (
        (actor_kind = 'user' AND action_source IN ('trusted_ui', 'app_command'))
        OR (actor_kind = 'app_policy' AND action_source IN (
            'policy_engine', 'adapter', 'explicit_cleanup'
        ))
        OR (actor_kind = 'runtime' AND action_source = 'runtime_observer')
    ),
    CHECK (
        (family = 'project_added'
            AND project_id IS NOT NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL AND reason_code IS NULL
            AND action_kind = 'project_add' AND outcome = 'applied'
            AND ((actor_kind = 'user' AND action_source IN ('trusted_ui', 'app_command'))
                OR (actor_kind = 'app_policy' AND action_source = 'policy_engine')))
        OR (family = 'trust_change'
            AND project_id IS NOT NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL AND reason_code IS NULL
            AND ((actor_kind = 'user' AND action_source = 'trusted_ui')
                OR (actor_kind = 'app_policy' AND action_source = 'policy_engine'))
            AND ((action_kind = 'trust_grant' AND operation_id IS NOT NULL
                    AND outcome IN ('authorized', 'applied', 'failed'))
                OR (action_kind = 'trust_revoke' AND operation_id IS NULL
                    AND outcome = 'applied')))
        OR (family = 'command_approval'
            AND project_id IS NOT NULL AND approval_id IS NOT NULL
            AND terminal_id IS NULL AND subject_kind IS NULL
            AND risk_level IS NOT NULL AND reason_code IS NULL
            AND ((action_kind = 'command_request' AND operation_id IS NULL
                    AND outcome = 'requested' AND actor_kind = 'app_policy'
                    AND action_source = 'adapter')
                OR (action_kind IN ('command_approve', 'command_edit_and_approve')
                    AND operation_id IS NOT NULL
                    AND outcome IN ('authorized', 'applied', 'failed')
                    AND actor_kind = 'user' AND action_source = 'trusted_ui')
                OR (action_kind = 'command_reject' AND operation_id IS NULL
                    AND outcome = 'applied' AND actor_kind = 'user'
                    AND action_source = 'trusted_ui')
                OR (action_kind = 'command_cwd_mismatch' AND operation_id IS NULL
                    AND outcome = 'anomaly' AND actor_kind = 'app_policy'
                    AND action_source = 'adapter')))
        OR (family = 'managed_process_lifecycle'
            AND project_id IS NOT NULL AND agent_run_id IS NOT NULL
            AND approval_id IS NULL AND operation_id IS NOT NULL
            AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NOT NULL
            AND action_kind = 'managed_agent_launch'
            AND ((outcome = 'authorized' AND reason_code IS NULL
                    AND ((actor_kind = 'user'
                            AND action_source IN ('trusted_ui', 'app_command'))
                        OR (actor_kind = 'app_policy' AND action_source = 'policy_engine')))
                OR (outcome = 'started' AND terminal_id IS NOT NULL
                    AND reason_code IS NULL AND actor_kind = 'runtime'
                    AND action_source = 'runtime_observer')
                OR (outcome = 'failed' AND reason_code IS NOT NULL
                    AND actor_kind = 'runtime' AND action_source = 'runtime_observer')
                OR (outcome = 'terminated' AND terminal_id IS NOT NULL
                    AND reason_code IS NOT NULL AND actor_kind = 'runtime'
                    AND action_source = 'runtime_observer')))
        OR (family = 'plain_terminal_observation'
            AND project_id IS NOT NULL AND terminal_id IS NOT NULL
            AND agent_run_id IS NULL AND approval_id IS NULL AND operation_id IS NULL
            AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL
            AND action_kind = 'plain_terminal_lifecycle'
            AND actor_kind = 'runtime' AND action_source = 'runtime_observer'
            AND outcome IN ('started', 'failed', 'terminated')
            AND (outcome = 'started' OR reason_code IS NOT NULL))
        OR (family = 'paste_blocked'
            AND project_id IS NOT NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL AND reason_code = 'paste_policy'
            AND action_kind = 'terminal_paste' AND outcome = 'blocked'
            AND actor_kind = 'app_policy' AND action_source = 'policy_engine')
        OR (family = 'restricted_mode_blocked'
            AND project_id IS NOT NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL AND reason_code = 'restricted_mode'
            AND action_kind = 'restricted_feature' AND outcome = 'blocked'
            AND actor_kind = 'app_policy' AND action_source = 'policy_engine')
        OR (family = 'root_access_blocked'
            AND project_id IS NOT NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind IS NULL AND risk_level IS NULL
            AND adapter_profile_ref IS NULL
            AND reason_code IN ('root_escape', 'symlink_escape')
            AND action_kind = 'root_access' AND outcome = 'blocked'
            AND actor_kind = 'app_policy' AND action_source = 'policy_engine')
        OR (family = 'safe_close_decision'
            AND project_id IS NOT NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND (subject_kind IS NULL OR subject_kind = 'app_resource')
            AND risk_level IS NULL AND adapter_profile_ref IS NULL
            AND action_kind IN ('safe_close_terminate', 'safe_close_abandon', 'destructive_action')
            AND actor_kind = 'user' AND action_source IN ('trusted_ui', 'app_command')
            AND ((outcome IN ('authorized', 'applied', 'failed')
                    AND operation_id IS NOT NULL)
                OR (outcome = 'cancelled' AND operation_id IS NULL)))
        OR (family = 'sensitive_config_changed'
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND subject_kind IS NULL AND risk_level IS NULL AND adapter_profile_ref IS NULL
            AND reason_code = 'policy_changed'
            AND ((actor_kind = 'user' AND action_source = 'trusted_ui')
                OR (actor_kind = 'app_policy' AND action_source = 'policy_engine'))
            AND ((action_kind = 'config_policy_increase' AND operation_id IS NOT NULL
                    AND outcome IN ('authorized', 'applied', 'failed'))
                OR (action_kind = 'config_policy_reduce' AND operation_id IS NULL
                    AND outcome = 'applied')))
        OR (family = 'transcript_purge'
            AND project_id IS NOT NULL AND terminal_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind = 'transcript'
            AND risk_level IS NULL AND adapter_profile_ref IS NULL AND reason_code IS NULL
            AND action_kind = 'transcript_purge' AND outcome IN ('completed', 'failed')
            AND ((actor_kind = 'user' AND action_source IN ('trusted_ui', 'app_command'))
                OR (actor_kind = 'app_policy' AND action_source = 'explicit_cleanup')))
        OR (family = 'audit_store_recovery'
            AND project_id IS NULL
            AND terminal_id IS NULL AND agent_run_id IS NULL AND approval_id IS NULL
            AND operation_id IS NULL AND subject_kind = 'recovery_bundle'
            AND risk_level IS NULL AND adapter_profile_ref IS NULL
            AND reason_code = 'recovery_completed'
            AND action_kind = 'audit_store_recovery' AND outcome = 'completed'
            AND actor_kind = 'user' AND action_source IN ('trusted_ui', 'app_command'))
    )
)"#
        )
    };
}

pub(crate) use audit_events_v2_table_ddl;

/// RFC-013 Amendment 1: the current (v2) full-install DDL -- the v2 table
/// (via [`audit_events_v2_table_ddl`], named `audit_events` directly) plus
/// its indexes, in the same single `execute_batch`-shaped string
/// [`CREATE_SCHEMA_V1`] was before it, used by `create_current_schema` for
/// a fresh install. An existing v1 installation never executes this
/// constant directly; it reaches the same table shape via the `1 -> 2`
/// `MigrationStep` in `audit::migration` instead (built from the same
/// macro, named `audit_events_v2_rebuild` and renamed after copying every
/// row), which is why the convergence test compares the two paths'
/// resulting `sqlite_master` SQL rather than assuming they agree.
///
/// `concat!` cannot splice in a previously-defined `const` (only literals
/// and nested macro invocations that themselves reduce to literals), so
/// the table DDL is inlined here via [`audit_events_v2_table_ddl`] rather
/// than through an intermediate named constant -- there is still exactly
/// one place the table body's literal text is written, which is the
/// property that matters.
pub const CREATE_SCHEMA_V2: &str = concat!(
    "\n",
    audit_events_v2_table_ddl!("audit_events"),
    ";\n",
    r#"
CREATE INDEX audit_events_project_sequence
    ON audit_events(project_id, sequence DESC);
CREATE INDEX audit_events_operation_sequence
    ON audit_events(operation_id, sequence ASC);
CREATE UNIQUE INDEX audit_events_one_authorization_per_operation
    ON audit_events(operation_id) WHERE outcome = 'authorized';
CREATE INDEX audit_events_family_outcome_sequence
    ON audit_events(family, outcome, sequence DESC);
"#
);
