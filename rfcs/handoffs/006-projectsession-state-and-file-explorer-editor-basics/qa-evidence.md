# RFC-006 QA Evidence

Date: 2026-07-05

Scope: RFC-006 implementation through PR-006-F closeout preparation.

## Gate Commands

Observed passing:

```text
cargo fmt --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
```

Observed test result:

```text
cargo test --all-targets
166 passed; 0 failed
```

Additional targeted evidence observed:

```text
cargo test -p tekstide-core shell -- --list
20 tests, 0 benchmarks

cargo test -p tekstide-core content -- --list
31 tests, 0 benchmarks
```

Narrow fixed-string scan over touched app/shell/project/content modules returned no matches:

```text
rg -n "std::process::Command|Command::new|reqwest|ureq|std::net|dotenv|\\.env" \
  crates/tekstide-core/src/app.rs crates/tekstide-core/src/app \
  crates/tekstide-core/src/shell.rs crates/tekstide-core/src/shell \
  crates/tekstide-core/src/project.rs crates/tekstide-core/src/project \
  crates/tekstide-core/src/content.rs crates/tekstide-core/src/content
```

## Implemented Slices

- PR-006-A: root-bound file access policy.
- PR-006-B: bounded file explorer model.
- PR-006-C: text document buffer.
- PR-006-D: safe save and external change detection.
- PR-006-E: Content Mode shell-visible text workflow and bounded explorer integration.
- PR-006-F: QA evidence and closeout preparation.

## Commit Evidence

RFC-006 implementation commits after planning acceptance:

```text
78fdfa3 Implement RFC-006 root access policy
65b1289 Implement RFC-006 bounded explorer model
8b32f0e Implement RFC-006 text document buffer
e344464 Implement RFC-006 safe save and external change detection
486b32f Integrate RFC-006 content mode text workflow
d88d7b7 Integrate RFC-006 content explorer workflow
```

Planning/design commit:

```text
84b80d9 Accept RFC-006 editor/explorer planning
```

## Acceptance Traceability

- User can open a project and browse files:
  - `app::tests::add_project_from_path_validates_and_restricts_before_display`
  - `shell::tests::content_workspace_renders_bounded_explorer_scan_without_file_contents`
  - `project::root::explorer::tests::scanner_reads_one_directory_as_sorted_read_model`

- User can open, edit, and save a text file under root:
  - `shell::tests::text_document_workflow_is_visible_in_active_content_workspace`
  - `content::tests::open::opens_valid_utf8_file_as_clean_string_backed_document`
  - `content::tests::edit::edit_transitions_document_to_dirty_without_saving`
  - `content::tests::save::saving_dirty_document_writes_file_and_marks_document_clean`

- Dirty state is visible:
  - `shell::tests::text_document_workflow_is_visible_in_active_content_workspace`
  - `app::tests::active_text_document_dirty_state_updates_project_runtime_summary`
  - `project_board::tests::view_model_uses_runtime_summary_for_known_counts_and_attention`

- External modification is detected:
  - `content::tests::save::clean_external_change_enters_prompt_state_without_overwriting_buffer`
  - `content::tests::save::save_blocks_dirty_buffer_when_file_changed_externally`
  - `shell::tests::external_dirty_conflict_is_visible_without_overwriting_disk`

- Symlink escape does not silently open outside-root content:
  - `project::root::tests::file_access_blocks_symlink_escape`
  - `project::root::explorer::tests::scanner_labels_in_root_symlink_and_blocks_escaping_symlink_node`
  - `content::tests::open::editable_open_blocks_escaping_symlink_file`
  - `content::tests::save::save_revalidates_root_containment_before_writing`

- Content Mode does not introduce split panes:
  - `shell::tests::content_workspace_renders_bounded_explorer_scan_without_file_contents`
  - `shell::tests::text_document_workflow_is_visible_in_active_content_workspace`
  - shell evidence renders `content panes: 1`.

## Scenario Evidence

Covered by shell text harness tests:

- browse project root and render bounded explorer nodes;
- show collapsed `target` directory;
- avoid dumping file contents in explorer output;
- open valid UTF-8 file;
- reject invalid/binary-looking files with bounded messages;
- edit active document and show dirty state;
- save active document and clear dirty state;
- block dirty external overwrite and show conflict;
- keep active document when explorer scan fails;
- keep active dirty document when a later file open fails;
- force Content Mode from Terminal / Agent Immersion Mode for both explorer scan and text open.

Representative rendered labels covered by tests:

```text
content status: open
explorer: ready
active file: src/lib.rs
document: dirty
dirty files: 1
content panes: 1
```

## Security / Privacy Evidence

RFC-006 implementation does not add:

- process launch;
- PTY behavior;
- network clients;
- Git probing;
- LSP startup;
- formatter startup;
- workspace hooks or scripts;
- task execution;
- plugin loading;
- workspace AI profile/prompt loading;
- `.env` loading.

Restricted Mode remains represented by RFC-004 security summaries. RFC-006 user-initiated open/edit/save operations are local root-bound file operations and do not start workspace automation.

Security-relevant file-access decisions are tested through root policy and content operations:

- root escape blocked;
- symlink escape blocked;
- unsafe symlink save blocked;
- save-time root containment revalidated;
- external changes are not overwritten silently.

Ordinary user open/save operations are not broadly audit-logged in this RFC. Security-relevant file-access audit persistence remains audit-pending until the later durable audit RFC.

## Privacy / Evidence Boundaries

Test fixtures use temporary directories and synthetic file contents.

Shell evidence intentionally renders project-relative paths, status labels, counts, and node summaries. It does not render editable file contents in explorer evidence, and tests assert that fixture file contents are absent from explorer output.

Review packages contain repository source and RFC artifacts. They exclude `.git/`, `.git-exclude/`, local agent config, and `target/`.

## Migration Note

No persisted schema migration is introduced by RFC-006.

Runtime content state is held in `ProjectSession` and is not serialized. Existing recent-project persistence remains unchanged.

## Known Limitations / Follow-up

- The current executable remains a CLI/text harness. A real GUI editor widget and file tree are deferred.
- RFC-006 uses one active text document. Multi-document tabs and arbitrary editor splits are intentionally not implemented.
- Clean external changes enter a prompt/status state in core; no GUI reload/compare confirmation is implemented yet.
- Dirty external changes enter conflict and block save; overwrite confirmation UI is intentionally absent.
- File watchers are not implemented. Save/refresh detection is snapshot based.
- Save uses same-directory temp-file plus rename for normal files, but does not claim full preservation of permissions, ownership, extended attributes, parent-directory fsync, or crash-consistency guarantees.
- In-root symlink files can be opened, but save through symlinks is blocked conservatively.
- Explorer scans one directory with bounded child count. Full recursive indexing and search are deferred.
- Security/audit persistence for file-access decisions remains pending a later durable audit RFC.

## Closeout Decision

Implementation evidence is ready for review as an RFC-006 closeout candidate.

Recommended acceptance status: accepted with documented limitations above.
