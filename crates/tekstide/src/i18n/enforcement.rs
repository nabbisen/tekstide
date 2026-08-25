//! RFC-016 PR-016-E: enforcement. Four mechanical checks:
//!
//! 1. [`no_raw_string_literal_is_passed_to_text_anywhere_in_the_crate`] --
//!    the canonical home for "no user-facing string is hardcoded" in
//!    `crates/tekstide`. This absorbs PR-015-B's identical scan (response
//!    128), which lived in `shell::tests` only because `shell.rs` was
//!    this crate's one module when it was written. One mechanical check
//!    for one policy, not two that can drift apart -- the same lesson
//!    `text_safety` already taught this RFC once (`implementation-handoff.md`).
//!
//! 2. [`tekstide_core_hardcoded_strings_match_the_closed_exemption_list`] --
//!    `tekstide-core` "owns state and policy and never renders" (per
//!    `pr-016-e-enforcement.md`), so it has no `text(...)`-shaped call to
//!    hook a scan onto the way `tekstide` does. What it has instead is a
//!    small, fixed set of modules that *do* produce user-facing English --
//!    found by direct inspection while writing this scan, not assumed
//!    from the handoff's own four-site table, which undercounts by two
//!    files and one site. See [`CORE_EXEMPT_LITERALS`]'s doc comment.
//!
//! 3. [`i18n_never_re_exports_a_raw_fluent_type`] -- response 126's
//!    recommended Fluent-type-exposure guard, made mechanical. `tekstide`
//!    is `[[bin]]`-only (no `[lib]` target), so a `compile_fail` doctest
//!    is not available here; this is the automated check the module doc
//!    said did not exist yet when PR-016-D landed.
//!
//! 4. [`every_source_locale_key_resolves_in_every_shipped_locale`] and
//!    [`report_catalog_keys_unused_by_any_render_call`] -- catalog
//!    completeness and its advisory mirror.
//!
//! **Why `tekstide-core` is not scanned as a whole crate.** A blanket
//! "any string literal is a violation" scan over every `.rs` file in
//! `tekstide-core` was tried first, by inspection, before writing any
//! code: 43 non-test files contain a space-containing string literal --
//! SQL migration statements, ANSI/VT escape sequences, error `Display`
//! text, audit codes. A "closed" exemption list with that many entries
//! is not closed in any sense that keeps it reviewable; it is exactly
//! the noise-nobody-acts-on failure mode a real enforcement mechanism
//! exists to avoid. So this scan names specific files, not the crate --
//! a real, disclosed limitation: a brand new `tekstide-core` module that
//! starts producing user-facing text somewhere else would not be caught
//! until it is added to [`CORE_TARGET_FILES`] below.

use std::path::{Path, PathBuf};

use crate::i18n::{Catalog, CatalogArgs, LocalePreference};

fn tekstide_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn tekstide_core_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tekstide-core/src")
}

fn real_locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("src dir must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

// ---------------------------------------------------------------------
// 1. `crates/tekstide`: no raw string literal passed to `text(...)`.
// ---------------------------------------------------------------------

/// Same exemptions as PR-015-B established: `theme.rs` is the one
/// legitimate place a literal belongs (it defines the palette), and
/// `tests.rs` files legitimately construct fixtures. `enforcement.rs`
/// (this file) is exempt from its own scan for the same reason its
/// source literally manipulates the substring `"text("` to find the
/// calls this scan is looking for -- a self-referential false positive,
/// not a real violation.
fn is_tekstide_scan_exempt(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("theme.rs") | Some("tests.rs") | Some("enforcement.rs")
    )
}

fn scannable_tekstide_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs_files(&tekstide_src_dir(), &mut files);
    files.retain(|path| !is_tekstide_scan_exempt(path));
    files
}

fn contains_text_call_with_string_literal(line: &str) -> bool {
    let mut search_from = 0;
    while let Some(relative_index) = line[search_from..].find("text(") {
        let call_start = search_from + relative_index + "text(".len();
        let after_paren = line[call_start..].trim_start();
        if after_paren.starts_with('"') {
            return true;
        }
        search_from = call_start;
    }
    false
}

/// Canonical since PR-016-E. Ablation-verified (`qa-evidence.md`):
/// introduce a `text("literal")` call in a covered file, confirm this
/// fails naming the file and line, revert.
#[test]
fn no_raw_string_literal_is_passed_to_text_anywhere_in_the_crate() {
    for path in scannable_tekstide_files() {
        let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            assert!(
                !contains_text_call_with_string_literal(line),
                "{}:{} passes a string literal directly to text(...): {line}",
                path.display(),
                line_number + 1
            );
        }
    }
}

// ---------------------------------------------------------------------
// 2. `tekstide-core`: closed exemption list for known text-producing
//    modules.
// ---------------------------------------------------------------------

/// Files this scan actually reads. `shell.rs` is scanned for presence
/// only (see [`CORE_BLANKET_EXEMPT_FILES`]); the other two are scanned
/// literal-by-literal against [`CORE_EXEMPT_LITERALS`].
const CORE_TARGET_FILES: &[&str] = &["shell.rs", "project_board.rs", "project/metadata.rs"];

/// `shell.rs` holds `render_text` and its two helpers -- the pre-GUI
/// text harness RFC-015 PR-015-D investigated and deliberately kept
/// (it is the primary assertion mechanism for ~20 `tekstide-core::shell::
/// tests`; see `qa-evidence.md`). It carries on the order of 25-30
/// literal occurrences once every connective fragment (`" | "`, `" -> "`,
/// per-field labels) is counted individually -- enumerating each one
/// would produce an exemption list dominated by punctuation, not a
/// meaningful review surface. Exempted as a whole file instead: this
/// scan only confirms `render_text` still exists (the disposition's own
/// trigger), not each literal within it. Dies as a unit when
/// `render_text` is deleted -- whichever RFC refactors
/// `tekstide-core::shell::tests` off it owns that.
const CORE_BLANKET_EXEMPT_FILES: &[&str] = &["shell.rs"];

/// One entry per literal, keyed by (file, exact content) so a stale
/// exemption -- the file no longer contains that literal -- is
/// detectable in the same test that catches a new, unlisted one.
///
/// **Corrections to `pr-016-e-enforcement.md`'s four-site table, found
/// while enumerating this list by direct inspection rather than by
/// scanning:**
///
/// - **Site 4 is only partially live.** `ProjectBoardRow` has four
///   fields (`trust_label`, `security_mode_label`, `availability_label`,
///   `blocked_automation_labels`); `crates/tekstide/src/surface/board.rs`'s
///   `row_lines` reads exactly one of them (`trust_label`). The other
///   three are constructed but never rendered by the real GUI today --
///   `board.rs`'s own module doc slightly overstates this by grouping
///   all four as "rendered as-is."
/// - **The literal that actually reaches a user is not in
///   `project_board.rs` at all.** For an *open* project, `trust_label`
///   is `project.trust_state().label()` --
///   `tekstide_core::project::metadata::WorkspaceTrust::label()`, a file
///   the four-site table does not name. **RFC-032 PR-032-C**: a *recent,
///   unopened* project's `recent_project_row` used to hardcode
///   `"Restricted"` directly regardless of the project's real cached
///   trust state -- fixed to read `RecentProject.trust_state.label()`
///   the same way `active_project_row` reads the live field, so both
///   paths now go through the one producer
///   (`WorkspaceTrust::label()`) instead of two independent literal
///   sources that could disagree. `security_mode_label`'s literal
///   (`"Restricted Mode"`/`"Trusted Mode"`) similarly now comes from
///   `RestrictedModeSummary::from_trust` (`security.rs`, not one of this
///   scan's target files) rather than being hardcoded here either.
/// - **A fifth core site, not in the table**:
///   `ProjectBoardViewModel::from_app_state`'s `ProjectBoardEmptyState`
///   construction. Dormant: `board.rs`'s `empty_state_view` reads only
///   `.is_some()` and renders its own catalog-driven strings instead
///   (already documented in `en.ftl`'s own comment above
///   `project-board-empty-heading`). **RFC-038 PR-038-E**: this
///   construction held three literals (`"No projects yet."`, `"Add
///   Project"`, `"Open from path"`); the latter two named actions
///   nothing implemented, and were removed from the published API
///   along with the fields that held them (`primary_action`/
///   `secondary_action`, see `CHANGELOG.md`). Only `"No projects yet."`
///   (`heading`) remains, still dormant, still not fixed here.
/// - **`project/metadata.rs` also holds two dormant `.label()`
///   producers** not in the table: `ProjectOpenSurface::label()` and
///   `ProjectMode::label()`. Both are reachable only through
///   `render_active_project_workspace` (site 1) -- the real GUI's
///   `AppRoute::ActiveProjectWorkspace` route (`shell::
///   active_project_workspace_view`, since RFC-015 PR-015-E) selects a
///   catalog key directly from the `ProjectMode` enum, never calling
///   `.label()`, so neither producer is live.
///
/// These corrections were raised as an open scope question in the
/// RFC-016 handoff's own `qa-evidence.md`, per its instruction for site
/// 4 ("raise it as a scope question rather than absorbing it here").
/// The `trust_label`/`security_mode_label` two-literal-sources gap is
/// now closed (RFC-032 PR-032-C, above); the remaining dormant/
/// not-yet-live corrections (`CountDisplay`/`AttentionState` labels, the
/// empty-state strings, `ProjectOpenSurface`/`ProjectMode::label()`)
/// are unrelated to trust and still not fixed here.
const CORE_EXEMPT_LITERALS: &[CoreExemptSite] = &[
    // project_board.rs -- CountDisplay::label. Dormant: response 130's
    // scan (`no_count_display_or_attention_label_is_called_anywhere_in_
    // the_crate`) proves the GUI crate never calls it; its only other
    // caller is the exempt `render_project_board` (site 1).
    CoreExemptSite::dormant("project_board.rs", "not available"),
    CoreExemptSite::dormant("project_board.rs", "not implemented"),
    CoreExemptSite::dormant("project_board.rs", "unknown"),
    // project_board.rs -- AttentionState::label. Dormant for the same
    // reason; also feeds the unused `attention_label` and
    // `global_attention_summary` fields (neither read by `board.rs`).
    CoreExemptSite::dormant("project_board.rs", "Risk"),
    CoreExemptSite::dormant("project_board.rs", "Approval needed"),
    CoreExemptSite::dormant("project_board.rs", "Review"),
    CoreExemptSite::dormant("project_board.rs", "Failed"),
    CoreExemptSite::dormant("project_board.rs", "Running"),
    CoreExemptSite::dormant("project_board.rs", "Dirty"),
    CoreExemptSite::dormant("project_board.rs", "Calm"),
    // project_board.rs -- ProjectBoardEmptyState (`from_app_state`).
    // Dormant: `board.rs`'s `empty_state_view` never reads this field.
    // The fifth site, not in `pr-016-e-enforcement.md`'s table.
    //
    // RFC-038 PR-038-E: `"Add Project"`/`"Open from path"` removed from
    // this list -- `ProjectBoardEmptyState::primary_action`/
    // `secondary_action`, the fields that held them, were removed from
    // the published API entirely (they named two actions that were
    // never reachable from anywhere; see `CHANGELOG.md`), so those two
    // literals no longer exist in `project_board.rs` at all. `"No
    // projects yet."` (`heading`) stays -- that field was not removed.
    CoreExemptSite::dormant("project_board.rs", "No projects yet."),
    // project_board.rs -- `recent_project_row`'s `availability_label`.
    // Dormant: `board.rs` never reads this field.
    CoreExemptSite::dormant("project_board.rs", "Folder missing"),
    CoreExemptSite::dormant("project_board.rs", "Cannot read folder"),
    CoreExemptSite::dormant("project_board.rs", "Path changed"),
    // project_board.rs -- RFC-032 PR-032-C fixed both of
    // `recent_project_row`'s hardcoded literals (`"Restricted"` /
    // `"Restricted Mode"`) to read the real `RecentProject.trust_state`/
    // `RestrictedModeSummary::from_trust` output instead, so neither
    // literal exists in this file to exempt any more -- see the
    // corrected bullet above.
    // project/metadata.rs -- WorkspaceTrust::label. The literal that
    // actually reaches a user for an *open* project's `trust_label`.
    // Live, tracked, not fixed here.
    CoreExemptSite::live("project/metadata.rs", "Unknown"),
    CoreExemptSite::live("project/metadata.rs", "Restricted"),
    CoreExemptSite::live("project/metadata.rs", "Trusted"),
    CoreExemptSite::live("project/metadata.rs", "Revoked"),
    // project/metadata.rs -- ProjectOpenSurface::label. Dormant: only
    // reachable through the placeholder `ActiveProjectWorkspace` route.
    CoreExemptSite::dormant("project/metadata.rs", "Project Dashboard"),
    CoreExemptSite::dormant("project/metadata.rs", "Text Editor"),
    CoreExemptSite::dormant("project/metadata.rs", "Git Status"),
    CoreExemptSite::dormant("project/metadata.rs", "AgentRun Detail"),
    CoreExemptSite::dormant("project/metadata.rs", "Diff Review"),
    CoreExemptSite::dormant("project/metadata.rs", "Handoff Report"),
    CoreExemptSite::dormant("project/metadata.rs", "Trust Settings"),
    // Response 233: `ApprovalHistory` is the first `ProjectOpenSurface`
    // variant with a real render arm in `view()` -- but `label()` itself
    // is still not that arm's source of user-facing text
    // (`approval_history_view` reads the `approval-history-heading`
    // catalog key directly, not `open_surface.label()`), so this
    // literal's own reachability is unchanged: still only
    // `tekstide_core::shell::render_text`'s pre-GUI harness, same as
    // every other variant above.
    CoreExemptSite::dormant("project/metadata.rs", "Approval History"),
    // project/metadata.rs -- ProjectMode::label. Same reason.
    CoreExemptSite::dormant("project/metadata.rs", "Content Mode"),
    CoreExemptSite::dormant("project/metadata.rs", "Terminal / Agent Immersion Mode"),
];

struct CoreExemptSite {
    file: &'static str,
    literal: &'static str,
    disposition: Disposition,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Disposition {
    /// Constructed in `tekstide-core` but never read by the real GUI
    /// crate today -- dies alongside `render_text` (site 1) or when its
    /// own module gains a real caller, whichever comes first.
    Dormant,
    /// Reaches the shipped GUI today via `ProjectBoardRow::trust_label`.
    /// Raised as an open scope question (`qa-evidence.md`), not fixed
    /// in this slice -- `ProjectBoardRow` would need to expose the
    /// underlying enum instead of a pre-rendered string.
    Live,
}

impl CoreExemptSite {
    const fn dormant(file: &'static str, literal: &'static str) -> Self {
        Self {
            file,
            literal,
            disposition: Disposition::Dormant,
        }
    }

    const fn live(file: &'static str, literal: &'static str) -> Self {
        Self {
            file,
            literal,
            disposition: Disposition::Live,
        }
    }
}

/// Heuristic, not a full parse -- matches this crate's existing
/// convention (`shell::tests`'s colour/font-size scans). Per-line,
/// unescaped double-quote pairing. Verified by inspection that none of
/// [`CORE_TARGET_FILES`]'s non-blanket-exempt entries contain an escaped
/// quote or a multi-line string literal.
fn extract_string_literals(source: &str) -> Vec<String> {
    let mut literals = Vec::new();
    for line in source.lines() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        let mut in_string = false;
        let mut current = String::new();
        for c in line.chars() {
            if c == '"' {
                if in_string {
                    literals.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
                in_string = !in_string;
            } else if in_string {
                current.push(c);
            }
        }
    }
    literals
}

/// Review gate: ablation-verified two ways (`qa-evidence.md`) --
/// introduce an unlisted literal in a scanned file (new-violation
/// direction), and delete a listed exemption's literal from the file it
/// names (stale-exemption direction). Both fail, naming the file and
/// the literal.
#[test]
fn tekstide_core_hardcoded_strings_match_the_closed_exemption_list() {
    for &file in CORE_TARGET_FILES {
        if CORE_BLANKET_EXEMPT_FILES.contains(&file) {
            let path = tekstide_core_src_dir().join(file);
            let source = std::fs::read_to_string(&path).expect("target file must be readable");
            assert!(
                source.contains("pub fn render_text("),
                "{}: blanket-exempt as the pre-GUI text harness, but `render_text` is gone -- \
                 this exemption's own trigger no longer holds; revisit whether it is still needed",
                path.display()
            );
            continue;
        }

        let path = tekstide_core_src_dir().join(file);
        let source = std::fs::read_to_string(&path).expect("target file must be readable");
        let found = extract_string_literals(&source);

        let expected: Vec<&str> = CORE_EXEMPT_LITERALS
            .iter()
            .filter(|site| site.file == file)
            .map(|site| site.literal)
            .collect();

        for literal in &found {
            assert!(
                expected.contains(&literal.as_str()),
                "{}: found a hardcoded string literal not in the closed exemption list: {literal:?}. \
                 If this is genuinely user-facing text, route it through the catalog instead. If it \
                 is a deliberate, dispositioned exception, add it to CORE_EXEMPT_LITERALS and record \
                 the disposition in qa-evidence.md.",
                path.display()
            );
        }

        for exempt in &expected {
            assert!(
                found.iter().any(|literal| literal == exempt),
                "{}: exemption list names {exempt:?} but it no longer appears as a string literal \
                 in this file -- remove the stale exemption entry from CORE_EXEMPT_LITERALS.",
                path.display()
            );
        }
    }
}

/// Confirms every exemption entry's `file` names something actually in
/// [`CORE_TARGET_FILES`] and, for `Live` entries, that the disposition
/// text above stays attached to at least one real site -- a
/// transcription check on the list itself, independent of the crate
/// scan above.
#[test]
fn every_core_exempt_site_names_a_scanned_file() {
    for site in CORE_EXEMPT_LITERALS {
        assert!(
            CORE_TARGET_FILES.contains(&site.file),
            "CORE_EXEMPT_LITERALS names {:?} for a literal {:?}, but that file is not in \
             CORE_TARGET_FILES -- it would never actually be scanned",
            site.file,
            site.literal
        );
    }
    assert!(
        CORE_EXEMPT_LITERALS
            .iter()
            .any(|site| site.disposition == Disposition::Live),
        "at least one exemption (WorkspaceTrust::label, the sites that reach the shipped GUI) \
         must stay recorded as Live -- if this ever becomes empty, either the fix landed (update \
         qa-evidence.md) or a live site was silently relabelled dormant"
    );
}

// ---------------------------------------------------------------------
// 3. `i18n` never re-exports a raw Fluent type.
// ---------------------------------------------------------------------

/// Response 126's guard, made mechanical: neither of the two files that
/// import `fluent_bundle` may re-export `FluentArgs`/`FluentValue`, or
/// `fluent_bundle` itself, at any visibility a caller outside this
/// module could reach. Before this existed, `pub use fluent_bundle::
/// {FluentArgs, FluentValue};` compiled cleanly and let
/// `FluentValue::from(any_runtime_str)` bypass `CatalogArgs` entirely --
/// probed and confirmed real (module doc, i18n.rs). Ablation-verified
/// (`qa-evidence.md`): add such a re-export back, confirm this fails,
/// revert.
#[test]
fn i18n_never_re_exports_a_raw_fluent_type() {
    for file in ["i18n.rs", "i18n/catalog.rs"] {
        let path = tekstide_src_dir().join(file);
        let source = std::fs::read_to_string(&path).expect("i18n file must be readable");
        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            let re_exports_fluent = trimmed.starts_with("pub use fluent_bundle")
                || (trimmed.starts_with("pub use") && trimmed.contains("FluentArgs"))
                || (trimmed.starts_with("pub use") && trimmed.contains("FluentValue"));
            assert!(
                !re_exports_fluent,
                "{}:{} re-exports a raw fluent_bundle type -- this defeats CatalogArgs's \
                 number/untrusted/trusted_symbol boundary (see the module doc's account of \
                 the U+202E probe): {line}",
                path.display(),
                line_number + 1
            );
        }
    }
}

// ---------------------------------------------------------------------
// 4. Catalog completeness, and its advisory mirror.
// ---------------------------------------------------------------------

/// Every top-level Fluent message key in `ftl_text`, in source order.
/// Heuristic (no leading whitespace, not a comment, looks like
/// `identifier =`), not a real Fluent parse -- matches the "heuristic
/// scan, not a full parser" convention this module already uses.
/// Verified against `en.ftl`/`pl.ftl`'s real, checked-in content:
/// multi-line pattern continuations and selector arms are always
/// indented or `*`-prefixed, so they never match this shape.
fn message_keys(ftl_text: &str) -> Vec<String> {
    ftl_text
        .lines()
        .filter_map(|line| {
            let first = line.chars().next()?;
            if !first.is_ascii_alphabetic() {
                return None;
            }
            let eq_pos = line.find('=')?;
            let key = line[..eq_pos].trim();
            let looks_like_an_identifier = key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            looks_like_an_identifier.then(|| key.to_string())
        })
        .collect()
}

/// One value for every variable any `en.ftl` message references today
/// (`$count`, `$route`, `$status`, `$attention`, `$number`, `$slot`).
/// Fluent ignores arguments a pattern does not reference, so passing all
/// six to every key's lookup is safe and lets this test stay generic
/// across keys rather than needing per-key argument shapes. **A future
/// key introducing a new variable name needs an entry here too** -- if
/// it does not get one, that key's lookup will error on the missing
/// variable and this test will fail, which is the intended signal, not
/// a false alarm. RFC-017 PR-017-E's `session-bar-entry` (response 150's
/// fix) is what added `$number`/`$slot` here.
fn generic_args() -> CatalogArgs<'static> {
    CatalogArgs::new()
        .number("count", 1u32)
        .trusted_symbol("route", "project-board")
        .trusted_symbol("status", "unknown")
        .trusted_symbol("attention", "calm")
        .number("number", 1u32)
        .trusted_symbol("slot", "hidden")
        .trusted_symbol("reason", "limit")
        .number("limit", 1u32)
        .number("line_count", 1u32)
        // RFC-019 PR-019-B: `explorer-node-entry`'s four selectors and
        // `explorer-status-error`'s untrusted message. `name`/`message`
        // go through the real `quote_untrusted` here too, not a plain
        // string -- `CatalogArgs::untrusted` only accepts a `DisplayText`.
        .trusted_symbol("kind", "file")
        .untrusted(
            "name",
            &tekstide_core::text_safety::quote_untrusted("fixture.txt"),
        )
        .trusted_symbol("state", "available")
        .trusted_symbol("symlink", "none")
        .untrusted(
            "message",
            &tekstide_core::text_safety::quote_untrusted("fixture error"),
        )
        // RFC-019 PR-019-C: `editor-chrome`'s untrusted path. `state`
        // above is shared with `explorer-node-entry` -- "available" is
        // not one of `editor-chrome`'s own arms, so it falls through to
        // that message's `*[clean]` default, which is exactly what a
        // completeness check needs (some resolvable string), not a
        // property this fixture asserts a specific value for.
        // RFC-019 PR-019-D: `external-change-dialog-body`'s untrusted
        // path reuses this same `path` arg -- no new entry needed.
        .untrusted(
            "path",
            &tekstide_core::text_safety::quote_untrusted("fixture.txt"),
        )
        // RFC-019 PR-019-E: `external-change-dialog-body`'s `$reason`
        // reuses this same `reason` arg -- "limit" is not one of its two
        // arms (`conflict`/`external-changed`), so it falls through to
        // the message's own `*[external-changed]` default, the same
        // "some resolvable string, not a specific asserted value" shape
        // `state`="available" already uses for `editor-chrome` above.
        // RFC-006 Amendment 1: `editor-cursor`'s two trusted numbers.
        .number("line", 1u32)
        .number("column", 1u32)
        // RFC-022 PR-022-E: `approval-dialog-body`'s untrusted command/cwd
        // and trusted risk-level selector. `command`/`cwd` go through the
        // real `quote_untrusted` here too, matching `name`/`message`/`path`
        // above -- `CatalogArgs::untrusted` only accepts a `DisplayText`.
        .untrusted(
            "command",
            &tekstide_core::text_safety::quote_untrusted("fixture-command"),
        )
        .untrusted(
            "cwd",
            &tekstide_core::text_safety::quote_untrusted("/fixture/cwd"),
        )
        .trusted_symbol("risk", "low")
        // RFC-032: `trust-grant-dialog-symlink-notice`'s untrusted root
        // path -- `trust-grant-dialog-body`'s own `$path` reuses the
        // `path` arg already above, no new entry needed for it.
        .untrusted(
            "root_path",
            &tekstide_core::text_safety::quote_untrusted("/fixture/root-path"),
        )
        // PR-020-B: `agent-run-detail-window-partial`/`-full`'s three
        // trusted numbers -- real byte offsets/lengths, never untrusted
        // text, so `.number` is correct here the same way `line`/
        // `column` above are for `editor-cursor`.
        .number("shown_len", 1u64)
        .number("total_len", 1u64)
        .number("delivered_start", 1u64)
        // RFC-033 PR-033-C: `trust-settings-retained-transcripts`'s and
        // `transcript-purge-dialog-body`'s trusted byte count -- a real
        // count of bytes on disk, never untrusted text, the same
        // reasoning `shown_len`/`total_len` above already establish for
        // this file's other byte counts. `count` above (already 1u32)
        // covers both keys' transcript-count selector; no new entry
        // needed for it.
        .number("bytes", 1u64)
        // RFC-041 PR-041-B: `change-review-content-non-text`'s real byte
        // length and `change-review-content-error-too-large`'s length
        // against its own bound -- both real, trusted numbers (never
        // untrusted text), the same reasoning `shown_len`/`total_len`
        // above already establish for this file's other byte counts.
        .number("len", 1u64)
        .number("max", 1u64)
}

fn shipped_additional_locales() -> Vec<String> {
    let mut locales = Vec::new();
    for entry in std::fs::read_dir(real_locales_dir()).expect("locales dir must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("ftl")
            && let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
            && stem != "en"
        {
            locales.push(stem.to_string());
        }
    }
    locales
}

/// Completeness, proven against the real fallback machinery
/// (`Catalog::resolve`/`get_with_args`), not by asserting the property
/// architecturally. `pl.ftl` deliberately defines only 3 of `en.ftl`'s
/// 21 keys (RFC-016 §Non-Goals: translation is content work, not this
/// RFC's job) -- every key `pl.ftl` does not define must still resolve,
/// via the source-locale fallback, to something other than the bare key.
#[test]
fn every_source_locale_key_resolves_in_every_shipped_locale() {
    let source_ftl_text = std::fs::read_to_string(real_locales_dir().join("en.ftl"))
        .expect("source-locale catalog must be readable");
    let keys = message_keys(&source_ftl_text);
    assert!(
        keys.len() >= 20,
        "expected roughly twenty keys per pr-016-e-enforcement.md; found {} -- \
         message_keys()'s heuristic may have drifted from en.ftl's real syntax",
        keys.len()
    );

    let args = generic_args();
    for locale in shipped_additional_locales() {
        let catalog = Catalog::resolve(
            LocalePreference {
                cli_flag: Some(locale.clone()),
                ..LocalePreference::default()
            },
            Some(&real_locales_dir()),
        );
        assert_eq!(
            catalog.resolved_locale(),
            locale,
            "expected to resolve to {locale}, not fall back at the locale level -- \
             {locale}.ftl may be missing, unreadable, or fail to parse"
        );

        for key in &keys {
            let rendered = catalog.get_with_args(key, &args);
            assert_ne!(
                &rendered, key,
                "{locale}: key `{key}` fell through every fallback stage and rendered as its \
                 own key -- neither {locale}.ftl nor the en.ftl fallback has a working \
                 definition (check whether this key introduced a variable not in generic_args())"
            );
        }
    }
}

/// Advisory, per `task-breakdown-pr-plan.md` -- deliberately never
/// fails. Reports `en.ftl` keys no `.get(...)`/`.get_with_args(...)`
/// call in `crates/tekstide/src` references, the mirror image of the
/// no-hardcoded-string scan above (that one finds code that should use
/// the catalog and does not; this one finds catalog entries nothing
/// asks for). Run with `--nocapture` to see the report.
#[test]
fn report_catalog_keys_unused_by_any_render_call() {
    let source_ftl_text = std::fs::read_to_string(real_locales_dir().join("en.ftl"))
        .expect("source-locale catalog must be readable");
    let keys = message_keys(&source_ftl_text);

    let mut referenced = Vec::new();
    let mut files = Vec::new();
    collect_rs_files(&tekstide_src_dir(), &mut files);
    for path in files {
        let source = std::fs::read_to_string(&path).expect("scannable file must be readable");
        for key in &keys {
            let quoted = format!("\"{key}\"");
            if source.contains(&quoted) {
                referenced.push(key.clone());
            }
        }
    }

    let unused: Vec<&String> = keys
        .iter()
        .filter(|key| !referenced.contains(key))
        .collect();
    if unused.is_empty() {
        println!(
            "[i18n enforcement] every en.ftl key is referenced somewhere in crates/tekstide/src"
        );
    } else {
        println!(
            "[i18n enforcement] {} of {} en.ftl keys have no `.get(...)`/`.get_with_args(...)` \
             call anywhere in crates/tekstide/src: {unused:?}. Advisory only -- not a failure.",
            unused.len(),
            keys.len()
        );
    }
}
