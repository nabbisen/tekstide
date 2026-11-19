//! RFC-074 D1: the capability-boundary gate — PR-074-A, with Stages 1 and
//! 2 (PR-074-B) now landed.
//!
//! Derives `tekstide-core`'s own module graph from its source text and
//! asserts four structural bounds (three at PR-074-A; the fourth added by
//! PR-074-B, review 672's own recommendation). **No module moves and no
//! `use` is rewritten by this file itself** — PR-074-A's own deliverable
//! was the gate alone; the stages that move code and lower the bound live
//! in `domain`, `project` and `runtime::terminal`, not here.
//!
//! **RFC-077 Stage 6 added two more, of a different kind (2026-11-16).**
//! `approval -> agent` (`VerifiedCwd`) is a permanent residual, not a
//! stage: cutting it deletes a construction-privilege guarantee while
//! *improving* every bound above, which this graph-cycle measurement
//! cannot see either way (see `ARCHITECTURE.md`'s seventh invariant,
//! "a privilege-bearing type's dependency is the privilege"). The two
//! assertions below are the compile-time witness that decision requires:
//! they check the guarantee directly, by name, rather than through a
//! property the graph happens to track.
//!
//! **The measurement method is fixed by the RFC and is not this test's to
//! improve quietly** (RFC-074, "Why now, measured today"): production `.rs`
//! files under `crates/tekstide-core/src` only (`tests.rs`, `*_tests.rs` and
//! anything under a `tests/` directory excluded, `bin/` excluded — those are
//! standalone `[[bin]]` targets, not library modules reachable via `crate::`),
//! full-line comments stripped, one edge counted per occurrence of
//! `crate::<module>` naming a top-level module other than the file's own. A
//! different method would make the RFC's own numbers uncheckable against this
//! gate, which is the one thing the gate exists for.
//!
//! **Reads from `CARGO_MANIFEST_DIR`, not a repository root found by walking
//! up to `.git/`** (amendment, 2026-11-12): an unpacked `tekstide-core`
//! package has no `.git/` but does have `src/`, so this gate must run
//! correctly there too — the same by-construction limitation
//! `release-checklist.md`'s Package Smoke (b) already had to correct from one
//! test to two, which a third, avoidable one should not join.
//!
//! **24 top-level modules, plus `test_support` as a 25th, special one.**
//! `lib.rs` declares 24 `pub mod`s unconditionally (21 at the RFC's own
//! count, `+1` from RFC-077 D7.2's `recent`, `+2` from RFC-077 D7.3's
//! `root` and `explorer_tree` -- all three new top-level modules, not
//! renamed ones, since each was `project::X` before) and one more,
//! `test_support`, gated on `any(test, feature = "test-support")` —
//! present in source either way, since this gate reads source text
//! rather than compiling, and it is exactly the node D1's third
//! assertion is about reaching. `test_support` is graphed as the 25th
//! because it must be, not because the count is wrong.
//!
//! **RFC-077 D7.0 (review 696) added two more bounds, because
//! `MAX_LARGEST_SCC_SIZE` reaching 0 at Stage 9 left D7 with no witness:
//! an already-acyclic module does not become more acyclic by splitting.**
//! Bound 1, `MAX_PROJECT_CAPABILITY_SUBMODULES`, is the one every D7 stage
//! must lower — the count of capability submodules `project.rs` itself
//! declares, which falls by construction only when a capability actually
//! leaves (a compatibility `pub use` façade line is not a `mod`
//! declaration, so API compatibility costs this bound nothing). Bound 2,
//! `MAX_PROJECT_GENUINE_REACH` (D7.0's own `MAX_OWN_CODE_REFERENCE_WEIGHT`,
//! replaced by review 703 after D7.3 found it countable but not monotone
//! — see that constant's own doc), is the opposite polarity — no stage
//! may *raise* it, though not every stage need lower it — and excludes
//! both façade (`pub use`/`pub(crate) use`) lines and references to a
//! departed D7 capability (`DEPARTED_D7_CAPABILITIES`) from the count,
//! since a pure re-export names a module without reaching into it, and a
//! reference to a module that just left is the cost of naming it
//! honestly, not reach into a capability still inside `project`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use tekstide_core::approval::{
    AcceptedProposal, ApprovalCoordinator, ApprovalQueueLimits, ReceiveOutcome,
};
use tekstide_core::audit::AuditCoordinator;
use tekstide_core::domain::{AgentRunId, ProjectId};

/// The ratcheted bound (RFC-074 D1.1). Started at 10, PR-074-A's own
/// measured value — a ceiling, not a goal. **Lowered to 9 by PR-074-B
/// Stage 1** (`ProjectId` moved to `domain`). **Lowered to 8 by Stage 2**
/// (`test_support`'s two references to/from `runtime` both gone,
/// dropping it out of the strongly connected component entirely — not
/// merely shrinking it by one, since `test_support` is no longer reached
/// by anything in it and reaches nothing in it either). **Lowered to 7 by
/// RFC-077 Stage 3** (`security`'s own one reference to `project` —
/// `WorkspaceTrust`, D5 move 1: vocabulary, moved to `domain::trust` —
/// gone, dropping `security` out of the component the same way
/// `test_support` dropped out at Stage 2: it is no longer reached by
/// anything in it and reaches nothing in it but `domain`). **Lowered to 6
/// by RFC-077 Stage 4** (`content`'s four references to `project::root`'s
/// file-access types and policy, across `document.rs`/`open.rs`/
/// `save.rs`/`snapshot.rs`: the five vocabulary types moved to
/// `domain::file_access`, D5 move 1; the one behavioral dependency
/// (`document.rs` calling `ProjectFileAccessPolicy` through a concrete
/// `ProjectRootHandle`) inverted into a trait `content` defines and
/// `project::root` implements, D5 move 2 — two moves, not four
/// incoherent ones, because the four references turned out to need
/// exactly two different things). **Lowered to 5 by RFC-077 Stage 5**
/// (`approval`'s five references to `audit`'s command-recording API,
/// all one coherent need this time: `approval` asks `audit` to record a
/// command request, decision and outcome, five vocabulary types moved to
/// `domain::audit` or replaced with `approval`'s own opaque failure
/// description, D5 move 1; the whole behavioral dependency inverted into
/// `approval::CommandAuditRecorder`, implemented on `audit::
/// AuditCoordinator` **in `audit` itself** rather than in `project` --
/// `project` has no edge to `audit` to place it on without re-coupling
/// `audit` back into the component through `project`, while `audit`
/// already has one to `approval`, so implementing there adds no new
/// edge). `audit` leaves the component entirely: nothing else inside it
/// reached `audit` once `approval`'s own edge was gone. Every move in
/// the same commit as the bound change that is its own evidence.
///
/// **Stage 6 did not move this bound** (`approval -> project` and
/// `approval -> agent`, 5 → 4 as originally scoped): `approval -> agent`
/// is `VerifiedCwd`'s own construction-privilege guarantee and is kept
/// permanently (see `ARCHITECTURE.md`'s seventh invariant); only the
/// `approval -> project` half landed, as housekeeping, and alone it does
/// not shrink the component. **Lowered to 4 by RFC-077 Stage 7**, from
/// the re-derived path this residual requires: `transcript`'s four
/// references to `project` (three `ProjectId` imports, D5 move 1: each
/// already vocabulary in `domain`, just pointed at `project`'s own
/// backward-compatible re-export) and `runtime` (one grouped import of
/// `next_token_len`, the D2 token-boundary function `transcript::reader`
/// resynchronizes with) both gone. The `runtime` reference did not fit
/// D5's vocabulary move — the function is real escape-sequence-parsing
/// behaviour, not vocabulary, and duplicating it was rejected on the
/// documented precedent this crate has already been burned by once — so
/// `read_window` now takes the boundary function as a parameter instead
/// of naming `runtime`, and `project::read_transcript_window` (D5 move 3:
/// `project` already has an edge to both `transcript` and `runtime`, so
/// it is the one module that can wire them together without creating a
/// new edge) supplies the real one. `transcript` leaves the component
/// entirely: nothing else inside it reached `transcript` once both of
/// its own edges were gone.
///
/// **Lowered to 2 by RFC-077 Stage 8** (review 684, directed rather than
/// escalated once the question was reframed: not "relocate the
/// approval-channel-binding step" but "move only the assembly of the
/// runtime-typed value, leave the binding where it already is"):
/// `agent`'s one reference to `project` (`ProjectSession`'s `id`,
/// `trust_state`, `canonical_root_path` -- a genuine behavioral need,
/// D5 move 2, inverted into `agent::AgentRunLaunchProject`, implemented
/// on `ProjectSession` **in `project/session.rs` itself**, since
/// `project` already has an edge to `agent` there) and `agent`'s four
/// references to `runtime` (`TerminalDimensions`/`TerminalEnvironmentPolicy`,
/// D5 move 1: dependency-free vocabulary, moved to `domain::terminal`;
/// `TerminalLaunchSpec`/`AdapterApprovalConfig`, D5 move 3: `agent` now
/// produces a runtime-free `AgentTerminalLaunchIntent` instead of
/// constructing the real spec itself, and `project::session`'s own
/// `assemble_terminal_launch_spec` -- the one module with edges to both
/// `agent` and `runtime` -- builds it from the intent) all gone. `agent`
/// leaves the component entirely, and so does `approval` (nothing else
/// inside it reached either once these four references were gone).
///
/// **Lowered to 0 by RFC-077 Stage 9** (review 685's finding, directed
/// by review 685's response, "move the binding cluster to project"):
/// `runtime -> project`'s own two references (`runtime/terminal/launch.rs`'s
/// `ProjectSession`, D5 move 2: new `runtime::terminal::TerminalLaunchProject`,
/// implemented on `ProjectSession` in `project/session.rs`, the same
/// placement rule Stage 8 already used; `runtime/git.rs`'s three
/// dependency-free git-summary types, D5 move 1: moved to new
/// `domain::git_status`, re-exported from `project` unchanged, with the
/// two behavioral methods staying physically in `project/metadata.rs`
/// per Rust's own inherent-impl rule) landed first, as housekeeping
/// (`c97d4ef`) -- alone it left `{agent, approval}` at 2, since neither
/// bound on it.
///
/// `agent -> approval` -- three independently-naming points review 685
/// found past the one return type first suspected (`ApprovalChannelEndpoint`
/// itself, `AgentAdapterApprovalError`'s own definition, and
/// `AgentTerminalAdapterApproval`'s `token` field one level down inside
/// `AgentTerminalLaunchIntent`) -- does not fit D5 while the binding
/// stays in `agent`: `RunCapabilityToken`'s own privilege is *crate*-
/// scoped, not `agent`-scoped, so it constrains which downstream crate
/// can extract it, not which module in this crate names the type.
/// Directed resolution: move the whole binding cluster (the grouped
/// `approval` import, `AgentTerminalAdapterApproval`, `prepare_adapter_approval`'s
/// body, `AgentAdapterApprovalError`) from `agent::launch` into
/// `project::session`, alongside the spec assembly Stage 8 already put
/// there -- `project` is already the sole caller, already imports
/// `ApprovalChannelEndpoint`, already branches adapter-versus-shell, and
/// already assembles the spec carrying the result. `AgentTerminalAdapterApproval`
/// is deleted outright rather than genericized: `bind_adapter_approval`
/// (`project/session.rs`) now writes the socket path and token straight
/// onto the `TerminalLaunchSpec` `prepare_agent_run_launch` is already
/// assembling, so there is no gap left between "bind" and "attach to the
/// spec" for an intermediate `agent`-owned carrier to live in.
/// `AgentAdapterApprovalError` keeps its exact name and shape, moved
/// rather than rewritten, so `shell.rs`'s own refusal-symbol match still
/// names `AgentAdapterApprovalError::Bind(ApprovalChannelErrorReason::
/// SocketPathTooLong)` concretely (RFC-060 PR-060-D, response 113
/// Required 1) -- the reviewed UX distinction review 685 flagged as
/// load-bearing is kept, not traded.
///
/// **Measured, not the directing response's own anticipated value**:
/// the response's own text called this "bound 2 → 1, zero cycles,
/// acyclic" — but this gate's own `strongly_connected_components` (see
/// its doc comment below) filters out every singleton component before
/// taking "largest," so a genuinely acyclic graph measures 0, not 1, by
/// this file's own definition. Confirmed directly (`MAX_LARGEST_SCC_SIZE`
/// set to 0 and the gate test run before committing to it) rather than
/// taken on the response's word: zero components of size greater than
/// one remain anywhere in the graph (22 modules at the time this was
/// measured; 25 after RFC-077 D7.2's `recent` and D7.3's `root` +
/// `explorer_tree` -- an acyclic graph does not become cyclic by a
/// module gaining siblings, confirmed directly at D7.3 too, not merely
/// assumed: `root`'s own new edge to the top-level `content` module
/// does not close a cycle with `content.rs`'s edge back to `root`,
/// since `content.rs`'s owning top-level module is `project`).
/// `agent -> approval`
/// is now `tekstide-core`'s only residual cross-module edge between these
/// two modules in either direction remaining from the original SCC —
/// one directed edge is not a cycle.
const MAX_LARGEST_SCC_SIZE: usize = 0;

/// RFC-077 D7.0 (review 696): the ratcheted bound every D7 stage must
/// lower from here -- `MAX_LARGEST_SCC_SIZE` reached 0 at Stage 9 and
/// cannot witness D7 at all, since an acyclic module does not become
/// more acyclic by splitting. `project.rs` declared 11 production
/// capability submodules at D7.0 (`change_detection`, `content`, `diff`,
/// `explorer_tree`, `ignored_directories`, `metadata`, `recent`, `root`,
/// `runtime`, `session`, `watch`); **lowered to 10 by RFC-077 D7.2**
/// (`recent` out). **Lowered to 7 by RFC-077 D7.3** (`root` +
/// `explorer_tree` out, together -- `root` is the internal base every
/// other departing capability still needs, so splitting it from
/// `explorer_tree`, a single nine-reference edge, would only double the
/// churn; `ignored_directories` also leaves the count here as a side
/// effect, not a stage of its own -- its one constant moved to `domain`
/// as a prerequisite, see bound 2's own doc, and the now-empty file was
/// deleted rather than kept as 1 LOC of orchestration ceremony). Falls
/// to 3 when D7 is done (`session`, `runtime` stay with orchestration).
/// Asserted `==`, the same reasoning `MAX_LARGEST_SCC_SIZE` already
/// carries: a shrink without the bound moving in the same commit is as
/// much a lie as a growth.
const MAX_PROJECT_CAPABILITY_SUBMODULES: usize = 7;

/// RFC-077 D7.0 (review 696), REPLACED by review 703's required fix:
/// the modules a D7 stage has already moved out of `project`, named
/// explicitly -- not derived from the original `mod` declaration names,
/// because two of the names a capability keeps when it leaves
/// (`content`, `runtime`) collide with real, unrelated top-level
/// modules `project` genuinely still reaches (`crate::content` 3,
/// `crate::runtime` 3, as of D7.3). An exclusion list built from
/// original `mod` names would silently discount those as "departed"
/// and make the bound below unfalsifiable in exactly the direction
/// that matters. Grows by one name per departure, in the same commit as
/// the stage that earns it -- the same discipline the bound below
/// carries. **Hand-maintained, and fails loud in the right direction
/// if a departure is missed**: forgetting to add a name here makes
/// that stage's own split artefact count as genuine reach, *raising*
/// `genuine_reach_weight_by_module`'s measure and redding the ceiling
/// below -- a red that means "this list is stale," not "the bound is
/// wrong."
const DEPARTED_D7_CAPABILITIES: &[&str] = &["recent", "root", "explorer_tree"];

/// RFC-077 D7.0 (review 696), REPLACED by review 703's required fix:
/// `MAX_OWN_CODE_REFERENCE_WEIGHT`'s own must-not-rise ceiling (33 at
/// D7.0, raised to 34 at D7.3) counted two things that move in opposite
/// directions and called the sum one bound. Decomposed at review 703:
/// `project`'s 34 own-code references split into **28 genuine reach**
/// (`transcript` 8, `domain` 7, `agent` 4, `runtime` 3, `content` 3,
/// `close` 2, `approval` 1 -- real capabilities `project` still names)
/// and **6 split artefact** (`root` 4, `explorer_tree` 2 -- the file
/// that just departed, named explicitly where `super::` sufficed
/// before, exactly once per stage, as the honest alternative to leaving
/// a blind spot). The genuine half **falls** every departure (33 at
/// D7.0 -> 28 by D7.3, as each capability takes its own external reach
/// with it); the artefact half **rises** every departure, by
/// construction, the same way total references rise when a module
/// splits (§5a). A ceiling built from the sum rises on correct work,
/// which is the opposite of a guard. **This bound counts only the
/// genuine half**: own-code references whose target is not in
/// `DEPARTED_D7_CAPABILITIES` -- monotone in the direction D7 moves,
/// immune to the artefact, and it measures what D7 is actually about:
/// how far orchestration still reaches into real capabilities. **Must
/// fall or hold, never rise -- not a literal must-fall** (review 704's
/// own correction: a departing capability with no external references
/// of its own, like `watch`, moves this bound by exactly zero, the same
/// shape the submodule-count bound's own §5a arithmetic already
/// established for a different metric). Asserted `<=`, the same
/// polarity the replaced bound carried. 28 today: falls when the
/// departing capability carried external references of its own, holds
/// when it did not -- not predicted per stage, measured. `app` is next
/// densest at 17, unaffected, all own-code and none of it a
/// departed-capability reference.
const MAX_PROJECT_GENUINE_REACH: usize = 28;

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every top-level module name present in source: a directory directly under
/// `src/` (its own `.rs` mod-declaration file, e.g. `domain.rs` beside
/// `domain/`, is the same module, not a second one), or a standalone
/// top-level `.rs` file with no matching directory (`command`, `route`,
/// `test_support`). `lib.rs` and `bin/` are not modules of the graph.
fn all_top_level_modules(root: &Path) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    let entries = fs::read_dir(root).expect("crates/tekstide-core/src must exist");
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "bin" {
            continue;
        }
        if path.is_dir() {
            modules.insert(name);
        } else if let Some(stem) = name.strip_suffix(".rs")
            && stem != "lib"
        {
            modules.insert(stem.to_owned());
        }
    }
    modules
}

/// `relative` (a path relative to `src/`) is excluded from the scan: inside a
/// directory literally named `tests`, named exactly `tests.rs`, or named
/// `*_tests.rs`.
fn is_excluded_from_scan(relative: &Path) -> bool {
    if relative
        .components()
        .any(|component| component.as_os_str() == "tests")
    {
        return true;
    }
    let Some(name) = relative.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name == "tests.rs" || name.ends_with("_tests.rs")
}

fn is_under_bin(relative: &Path) -> bool {
    relative
        .components()
        .next()
        .is_some_and(|component| component.as_os_str() == "bin")
}

/// The top-level module a file under `src/` belongs to: the first path
/// component, with a trailing `.rs` stripped when that component is the file
/// itself (a file directly under `src/`, not inside a subdirectory).
fn owning_module(relative: &Path) -> String {
    let first = relative
        .components()
        .next()
        .expect("a file under src/ has at least one path component")
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    first.strip_suffix(".rs").unwrap_or(&first).to_owned()
}

fn production_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_rs_files(root, root, &mut out);
    out
}

fn collect_rs_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .expect("walked path is under root")
            .to_owned();
        if is_under_bin(&relative) {
            continue;
        }
        if path.is_dir() {
            collect_rs_files(root, &path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs")
            && !is_excluded_from_scan(&relative)
        {
            out.push(path);
        }
    }
}

/// `source` with every full-line comment removed — a line whose trimmed text
/// starts with `//` (covering `///` and `//!` too). **Deliberately not every
/// comment**: the RFC's own method strips "full-line comments," not a
/// trailing one on a code line, and this gate must measure exactly what the
/// RFC's own numbers were measured with.
fn strip_full_line_comments(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `crate::` occurrence in `source`, paired with the byte offset and
/// whatever identifier — possibly empty — immediately follows it.
/// `crate_path_references`, below, keeps only the occurrences whose
/// identifier is a known module; this keeps every one, including the
/// occurrences that identifier check would otherwise make disappear with no
/// record at all. Required by review 686 Required 1(b): `crate::{`,
/// `crate ::`, a line break after `crate::`, and an unknown re-export all
/// produce an empty or unrecognized identifier here, where
/// `every_crate_path_occurrence_names_a_known_top_level_module` can see and
/// reject them, instead of silently vanishing inside `crate_path_references`.
fn crate_path_occurrences(source: &str) -> Vec<(usize, String)> {
    const PREFIX: &str = "crate::";
    let mut found = Vec::new();
    let mut search_from = 0usize;
    while let Some(at) = source[search_from..].find(PREFIX) {
        let ident_start = search_from + at + PREFIX.len();
        let ident: String = source[ident_start..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        found.push((ident_start, ident));
        search_from = ident_start;
    }
    found
}

/// Every occurrence of `crate::<module>` in `source` where `<module>` is one
/// of `known_modules`, as that module's name — one entry per occurrence, not
/// deduplicated (the RFC's own "references" count, as opposed to "edges").
fn crate_path_references(source: &str, known_modules: &BTreeSet<String>) -> Vec<String> {
    crate_path_occurrences(source)
        .into_iter()
        .map(|(_, ident)| ident)
        .filter(|ident| known_modules.contains(ident))
        .collect()
}

/// The measured graph: every cross-module reference, `(from, to)`, counted
/// once per occurrence (not deduplicated — that is what distinguishes the
/// RFC's own "edges" count from its "references" count).
struct CapabilityGraph {
    modules: BTreeSet<String>,
    references: Vec<(String, String)>,
}

impl CapabilityGraph {
    /// Distinct `(from, to)` pairs — the RFC's own "edges," as opposed to the
    /// raw, possibly-repeated `references` count.
    fn edges(&self) -> BTreeSet<(String, String)> {
        self.references.iter().cloned().collect()
    }

    /// The distinct modules `from` has an outgoing edge to.
    fn outgoing_targets(&self, from: &str) -> BTreeSet<String> {
        self.edges()
            .into_iter()
            .filter(|(f, _)| f == from)
            .map(|(_, t)| t)
            .collect()
    }

    /// The distinct modules with an outgoing edge to `to`, other than `to`
    /// itself.
    fn modules_reaching(&self, to: &str) -> BTreeSet<String> {
        self.edges()
            .into_iter()
            .filter(|(f, t)| t == to && f != to)
            .map(|(f, _)| f)
            .collect()
    }

    /// Every strongly connected component with more than one member —
    /// Tarjan's algorithm over the deduplicated edge set. A singleton module
    /// with no self-loop is not "strongly connected" to anything and is
    /// excluded, matching the RFC's own framing ("one strongly connected
    /// component of ten" — not twenty-two trivial ones).
    fn strongly_connected_components(&self) -> Vec<BTreeSet<String>> {
        let nodes: Vec<&str> = self.modules.iter().map(String::as_str).collect();
        let index_of: BTreeMap<&str, usize> =
            nodes.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
        for (from, to) in self.edges() {
            adjacency[index_of[from.as_str()]].push(index_of[to.as_str()]);
        }

        let mut tarjan = Tarjan::new(nodes.len());
        for node in 0..nodes.len() {
            if tarjan.index[node].is_none() {
                tarjan.strong_connect(node, &adjacency);
            }
        }
        tarjan
            .components
            .into_iter()
            .filter(|component| component.len() > 1)
            .map(|component| component.into_iter().map(|i| nodes[i].to_owned()).collect())
            .collect()
    }
}

/// A textbook, iterative-call-stack-free (recursive is fine — 22 nodes)
/// Tarjan's strongly-connected-components algorithm.
struct Tarjan {
    index: Vec<Option<usize>>,
    low_link: Vec<usize>,
    on_stack: Vec<bool>,
    stack: Vec<usize>,
    next_index: usize,
    components: Vec<Vec<usize>>,
}

impl Tarjan {
    fn new(node_count: usize) -> Self {
        Self {
            index: vec![None; node_count],
            low_link: vec![0; node_count],
            on_stack: vec![false; node_count],
            stack: Vec::new(),
            next_index: 0,
            components: Vec::new(),
        }
    }

    fn strong_connect(&mut self, node: usize, adjacency: &[Vec<usize>]) {
        self.index[node] = Some(self.next_index);
        self.low_link[node] = self.next_index;
        self.next_index += 1;
        self.stack.push(node);
        self.on_stack[node] = true;

        for &neighbour in &adjacency[node] {
            if self.index[neighbour].is_none() {
                self.strong_connect(neighbour, adjacency);
                self.low_link[node] = self.low_link[node].min(self.low_link[neighbour]);
            } else if self.on_stack[neighbour] {
                self.low_link[node] = self.low_link[node].min(self.index[neighbour].unwrap());
            }
        }

        if self.low_link[node] == self.index[node].unwrap() {
            let mut component = Vec::new();
            loop {
                let member = self.stack.pop().expect("component root is on the stack");
                self.on_stack[member] = false;
                component.push(member);
                if member == node {
                    break;
                }
            }
            self.components.push(component);
        }
    }
}

fn measure_capability_graph() -> CapabilityGraph {
    let root = src_root();
    let modules = all_top_level_modules(&root);
    let mut references = Vec::new();
    for file in production_files(&root) {
        let relative = file.strip_prefix(&root).expect("under src/");
        let from = owning_module(relative);
        let source =
            fs::read_to_string(&file).unwrap_or_else(|e| panic!("reading {}: {e}", file.display()));
        let stripped = strip_full_line_comments(&source);
        for to in crate_path_references(&stripped, &modules) {
            if to != from {
                references.push((from.clone(), to));
            }
        }
    }
    CapabilityGraph {
        modules,
        references,
    }
}

/// RFC-077 D7.0 (review 696): whether `line`'s trimmed text is a pure
/// re-export (`pub use ...` or `pub(crate) use ...`) -- the façade rule
/// §5 of the scoping document recommends and the owner authorized: a
/// line that only re-exports a name does not reach into the module it
/// names, so it is excluded from the own-code reference weight bound.
/// Whole-line classification, matching every other measurement in this
/// file (`is_excluded_from_scan`, `declares_a_module_with_a_body`):
/// no production line in this crate mixes a façade re-export with a
/// real, behavioral use of `crate::` on the same line.
fn is_facade_reexport_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("pub use ") || trimmed.starts_with("pub(crate) use ")
}

/// RFC-077 D7.0 (review 696), bound 2: own-code reference weight per
/// module -- the same per-occurrence counting `measure_capability_graph`
/// already does, attributed to the source module instead of built into
/// the cross-module edge list, with every façade line
/// (`is_facade_reexport_line`) excluded first.
///
/// **Sound only because `crate::` never wraps across a line break in this
/// crate (review 697 round 2 finding).** This scans `crate_path_references`
/// **per line**, unlike `measure_capability_graph`'s whole-file scan --
/// equivalent only because a `crate::` whose identifier wrapped to the next
/// line would read as an empty identifier either way, and
/// `every_crate_path_occurrence_names_a_known_top_level_module` already
/// reds on exactly that shape, so that case cannot exist uncaught. If that
/// guard is ever weakened or removed, this function's own soundness goes
/// with it, silently.
fn own_code_reference_weight_by_module() -> BTreeMap<String, usize> {
    let root = src_root();
    let modules = all_top_level_modules(&root);
    let mut weight: BTreeMap<String, usize> = BTreeMap::new();
    for file in production_files(&root) {
        let relative = file.strip_prefix(&root).expect("under src/");
        let from = owning_module(relative);
        let source =
            fs::read_to_string(&file).unwrap_or_else(|e| panic!("reading {}: {e}", file.display()));
        let stripped = strip_full_line_comments(&source);
        for line in stripped.lines() {
            if is_facade_reexport_line(line) {
                continue;
            }
            for to in crate_path_references(line, &modules) {
                if to != from {
                    *weight.entry(from.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    weight
}

/// RFC-077 D7.0 (review 696), bound 2's replacement (review 703
/// Required): `own_code_reference_weight_by_module`'s own counting,
/// with one more exclusion -- a reference whose target is a departed
/// D7 capability (`DEPARTED_D7_CAPABILITIES`) does not count as
/// "genuine reach," since it exists only because a capability that
/// used to share a top-level module with its caller now has to be
/// named explicitly. This is the artefact review 703 found; excluding
/// it is what makes the bound this feeds monotone in the direction D7
/// moves, rather than rising on every departure the way the replaced
/// ceiling did.
fn genuine_reach_weight_by_module() -> BTreeMap<String, usize> {
    let root = src_root();
    let modules = all_top_level_modules(&root);
    let mut weight: BTreeMap<String, usize> = BTreeMap::new();
    for file in production_files(&root) {
        let relative = file.strip_prefix(&root).expect("under src/");
        let from = owning_module(relative);
        let source =
            fs::read_to_string(&file).unwrap_or_else(|e| panic!("reading {}: {e}", file.display()));
        let stripped = strip_full_line_comments(&source);
        for line in stripped.lines() {
            if is_facade_reexport_line(line) {
                continue;
            }
            for to in crate_path_references(line, &modules) {
                if to != from && !DEPARTED_D7_CAPABILITIES.contains(&to.as_str()) {
                    *weight.entry(from.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    weight
}

/// RFC-077 D7.0 (review 696), control 1: `module`'s total reference
/// count exactly as `measure_capability_graph` counts it -- façade lines
/// included -- so the own-code weight's own subtraction (total minus
/// façade) is a number this gate measures separately, rather than one
/// trusted because the own-code figure alone looks right.
fn total_reference_weight(graph: &CapabilityGraph, module: &str) -> usize {
    graph
        .references
        .iter()
        .filter(|(from, _to)| from == module)
        .count()
}

/// RFC-077 D7.0 (review 696), bound 1: the count of production
/// capability-submodule declarations `project.rs` itself makes --
/// `mod <name>;` / `pub mod <name>;`, never a body
/// (`no_production_file_declares_a_module_with_a_body` already holds
/// there are none in this file) -- excluding one gated by `#[cfg(test)]`
/// on its own immediately preceding non-blank line. **Off-by-one this
/// closes deliberately**: a naive count of every `mod` line returns 12,
/// since `#[cfg(test)] mod tests;` is the twelfth -- excluded by the
/// attribute, not by the module's name, the same class review 687's own
/// correction closed for a different file.
fn project_capability_submodule_count() -> usize {
    let root = src_root();
    let path = root.join("project.rs");
    let source = fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path:?}: {e}"));
    let mut count = 0;
    let mut previous_non_blank: Option<&str> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if module_declaration(trimmed).is_some() {
            let gated_by_cfg_test = previous_non_blank == Some("#[cfg(test)]");
            if !gated_by_cfg_test {
                count += 1;
            }
        }
        previous_non_blank = Some(trimmed);
    }
    count
}

/// Renamed from `..._has_at_most_ten_modules` when Stage 1 lowered the
/// bound to 9 and stale again at Stage 2's 8 -- the name embedding the
/// bound's own value is exactly the state-asserting-text failure this
/// project's documents are written against, now that the bound is meant
/// to keep moving.
#[test]
fn the_largest_strongly_connected_component_is_within_its_ratcheted_bound() {
    let graph = measure_capability_graph();
    let components = graph.strongly_connected_components();
    let largest = components
        .iter()
        .max_by_key(|component| component.len())
        .cloned()
        .unwrap_or_default();
    // RFC-077 Stage 9: `MAX_LARGEST_SCC_SIZE` reached 0, `usize`'s own
    // minimum, once the graph went fully acyclic -- `<=` against it is
    // clippy's `absurd_extreme_comparisons` (always true), and the
    // ratchet never raises this bound anyway, so `==` says the same
    // thing the comparison always meant, without tripping it.
    assert!(
        largest.len() == MAX_LARGEST_SCC_SIZE,
        "the largest strongly connected component has {} modules ({largest:?}); the ratcheted \
         bound is {MAX_LARGEST_SCC_SIZE} (RFC-074 D1.1) — if this grew, a stage was undone; if it \
         shrank without the bound moving with it in the same commit, the bound is now a lie",
        largest.len()
    );
}

/// RFC-077 D7.0 (review 696), bound 1: `project.rs`'s own capability-
/// submodule count, the witness `MAX_LARGEST_SCC_SIZE` cannot provide
/// once it reaches 0 -- splitting an already-acyclic module does not
/// change its cycle structure, so D7 needs a bound that falls by
/// construction with every capability that actually leaves. Falls to 3
/// when D7 is done.
#[test]
fn project_capability_submodule_count_is_within_its_ratcheted_bound() {
    let count = project_capability_submodule_count();
    assert!(
        count == MAX_PROJECT_CAPABILITY_SUBMODULES,
        "project.rs declares {count} production capability submodules; the ratcheted bound is \
         {MAX_PROJECT_CAPABILITY_SUBMODULES} (RFC-077 D7.0) -- if this grew, a submodule was \
         added without the bound moving with it; if it shrank without the bound moving in the \
         same commit, a D7 stage landed without lowering its own witness"
    );
}

/// RFC-077 D7.0 (review 696), bound 2, REPLACED by review 703's
/// required fix: no module's *genuine-reach* weight (references to a
/// module that has not left `project` in a D7 stage) may exceed this
/// ceiling. The replaced test asserted the same shape against raw
/// own-code weight, which counted a split artefact that rises every
/// departure -- see `MAX_PROJECT_GENUINE_REACH`'s own doc for the
/// decomposition. This bound must fall or hold, never rise; RFC-074's
/// stages 6 through 9 already showed this gate will reward lowering one
/// bound by densifying a hub unless something else refuses to let the
/// hub get denser, and this is that something, now pointed at a metric
/// the hub's own split cannot inflate.
///
/// **This assertion alone is vacuous against a blind instrument (review
/// 697 round 2 finding), the same shape review 686 required a positive
/// control for at the SCC bound.** `max_by_key(...).unwrap_or_default()`
/// reads an empty weight map as `0 <= 28` and passes -- so if
/// `genuine_reach_weight_by_module` ever stopped measuring anything,
/// this test would too, silently. `projects_three_reference_numbers_are_
/// pinned_before_the_facade_exclusion`'s own `own == 34` is what actually
/// catches that case, by pinning a nonzero value directly; its doc says so
/// too, so the dependency is written down at both ends. (The pin tracks
/// raw own-code, not genuine reach, since it is the measurement record
/// bound 2's replacement was decomposed from -- it still pins a nonzero
/// value, which is all the positive control needs.)
#[test]
fn no_module_exceeds_its_genuine_reach_ceiling() {
    let weight = genuine_reach_weight_by_module();
    let (densest_module, densest_weight) = weight
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(module, count)| (module.clone(), *count))
        .unwrap_or_default();
    assert!(
        densest_weight <= MAX_PROJECT_GENUINE_REACH,
        "{densest_module} carries {densest_weight} genuine-reach references (departed D7 \
         capabilities excluded), exceeding the ceiling of {MAX_PROJECT_GENUINE_REACH} \
         (RFC-077 D7.0, replaced by review 703) -- a D7 stage made some module reach further \
         into a real, undeparted capability, which is exactly the trade stages 6 through 9 \
         already took against the SCC bound and this guard exists to refuse doing again here: \
         {weight:?}"
    );
}

/// RFC-077 D7.0 (review 696), control 1: the three numbers bound 2's own
/// façade exclusion rests on, pinned separately so the subtraction
/// (total minus façade equals own-code) is measured rather than
/// trusted.
///
/// **These three numbers are ratcheted, the same as `MAX_LARGEST_SCC_SIZE`
/// and `MAX_PROJECT_CAPABILITY_SUBMODULES` above (review 697 Required).**
/// Every D7 stage that moves a capability out of `project` moves at
/// least one of them -- but **not by the naive prediction**, measured
/// rather than assumed at D7.2: the staging document's own §5a
/// arithmetic predicted `recent` alone would take total 37 → 36, own
/// 33 → 32, façade 4 → 5. Own and façade landed exactly there; **total
/// did not move at all, still 37** -- the prediction only subtracted the
/// one reference `recent` itself carries away (its own
/// `timestamp.rs -> domain` line, previously counted under `project`
/// because `recent` was nested inside it); it did not add back the one
/// new reference `project.rs`'s own replacement line creates
/// (`pub use crate::recent;`, now a real `crate::` occurrence where
/// `mod recent;` was invisible to this gate). The two cancel exactly
/// here; a future stage's own net change on `total` is not derivable
/// from the submodule-count delta alone, only from measuring directly.
/// **D7.3 moved all three again, past what review 702's own response
/// predicted (~29 own-code, assuming only two files would newly
/// cross)**: measured total 41, own 34, façade 7. `MAX_PROJECT_
/// GENUINE_REACH`'s own doc has the full accounting for why raw
/// own-code rose instead of falling, and why that discovery replaced
/// bound 2 rather than merely explaining it -- 34 decomposes into 28
/// genuine reach (which fell, as every departure does) and 6 split
/// artefact (`root` 4, `explorer_tree` 2 -- the departed module named
/// explicitly where `super::` sufficed before). **This pin keeps
/// tracking raw own-code, not genuine reach**: it is the measurement
/// record bound 2's replacement was decomposed from, and review 703
/// said so directly ("keep the pin as the measurement record; it is
/// doing its job"). **A red here is this test working, not a
/// regression**: update the three literals in the same commit as the
/// stage that earns the new values, exactly as a stage must move
/// `MAX_PROJECT_CAPABILITY_SUBMODULES` and `MAX_PROJECT_GENUINE_REACH`.
/// This pin also doubles as `no_module_exceeds_its_genuine_reach_
/// ceiling`'s own positive control (a nonzero own-code value pinned
/// directly) -- see that test's doc.
#[test]
fn projects_three_reference_numbers_are_pinned_before_the_facade_exclusion() {
    let graph = measure_capability_graph();
    let total = total_reference_weight(&graph, "project");
    let own_code_weight = own_code_reference_weight_by_module();
    let own = own_code_weight.get("project").copied().unwrap_or(0);
    assert_eq!(
        total, 41,
        "project's total reference count (façade included) moved"
    );
    assert_eq!(
        own, 34,
        "project's own-code reference count (façade excluded) moved"
    );
    assert_eq!(
        total - own,
        7,
        "project's façade (pub use / pub(crate) use) reference count moved"
    );
}

/// RFC-077 D7.0 (review 696), control 2, in review 686's own shape: the
/// façade exclusion must not be able to widen silently into "nothing
/// counts" -- assert a known façade line **is** excluded and a known
/// own-code reference **is** still counted, both located by searching
/// `project.rs`'s real, current text rather than assumed still present.
#[test]
fn the_facade_exclusion_drops_a_known_reexport_and_keeps_a_known_own_code_reference() {
    let root = src_root();
    let path = root.join("project.rs");
    let source = fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path:?}: {e}"));
    let stripped = strip_full_line_comments(&source);

    let facade_line = stripped
        .lines()
        .find(|line| line.trim_start() == "pub use crate::domain::ProjectId;")
        .expect("project.rs must still re-export ProjectId from domain unchanged");
    assert!(
        is_facade_reexport_line(facade_line),
        "a known façade line stopped being recognized as one: {facade_line:?}"
    );

    let own_code_line = stripped
        .lines()
        .find(|line| line.contains("crate::transcript::read_window("))
        .expect("read_transcript_window must still call crate::transcript::read_window directly");
    assert!(
        !is_facade_reexport_line(own_code_line),
        "a known own-code reference was misclassified as a façade line: {own_code_line:?}"
    );
    let modules = all_top_level_modules(&root);
    assert!(
        crate_path_references(own_code_line, &modules).contains(&"transcript".to_owned()),
        "the known own-code reference line no longer names crate::transcript at all: \
         {own_code_line:?}"
    );
}

/// RFC-077 review 686 Required 1(a): a positive control over the whole
/// measurement pipeline (module enumeration, `production_files`,
/// `owning_module`, `crate_path_references`) — not a guarantee about
/// `VerifiedCwd` itself, which `ARCHITECTURE.md`'s seventh invariant's own
/// three guards already hold (`verified_cwd_from_validated_has_exactly_one_production_call_site`,
/// `receive_proposal_still_requires_a_verified_cwd_not_a_raw_path`,
/// `receive_proposal_verified_cwd_type_pin`). At `MAX_LARGEST_SCC_SIZE ==
/// 0`, every other graph assertion in this file is "this set is empty" —
/// the SCC bound above, `domain_has_zero_outgoing_cross_module_references`,
/// and both `test_support` assertions below — and an instrument that saw
/// nothing would pass every one of them exactly as a genuinely acyclic
/// graph does. `approval -> agent` is the one edge this crate keeps
/// permanently; if the measured edge set ever stops containing it, the
/// pipeline went blind, which this assertion exists to catch instead of
/// reading as a cleaner graph. Demonstrated, not hypothetical: rewriting
/// `agent/launch.rs`'s single `use crate::approval::{...}` into the
/// equally idiomatic `use crate::{approval::{...}}` removes the edge from
/// what `crate_path_references` sees without moving any code — see
/// `every_crate_path_occurrence_names_a_known_top_level_module`, below,
/// for the assertion that closes that class rather than this one instance.
#[test]
fn the_measured_graph_still_sees_the_permanent_residual_edge() {
    let graph = measure_capability_graph();
    let edges = graph.edges();
    assert!(
        edges.contains(&("approval".to_owned(), "agent".to_owned())),
        "the measured graph no longer contains approval -> agent — this crate's one permanent \
         residual edge (VerifiedCwd's construction privilege, ARCHITECTURE.md's seventh \
         invariant). At bound 0 every other assertion in this file is \"this set is empty,\" so \
         losing this edge from the measurement reads as a cleaner graph instead of a broken \
         instrument unless this positive control catches it first"
    );
}

/// RFC-077 review 686 Required 1(b): closes the *class* review 686 found,
/// not only the one instance named above — `crate::{`, `crate ::`, a line
/// break immediately after `crate::`, and `crate::SomeRootReexport` would
/// all silently vanish from `crate_path_references` the same way, each
/// producing an identifier that is empty or not a known module, which that
/// function drops with no record at all. Measured across the whole tree
/// before being written, matching review 686's own count: 114 production
/// files, 141 `crate::` occurrences, zero exceptions.
#[test]
fn every_crate_path_occurrence_names_a_known_top_level_module() {
    let root = src_root();
    let modules = all_top_level_modules(&root);
    let mut unmatched = Vec::new();
    for file in production_files(&root) {
        let relative = file.strip_prefix(&root).expect("under src/").to_owned();
        let source =
            fs::read_to_string(&file).unwrap_or_else(|e| panic!("reading {}: {e}", file.display()));
        let stripped = strip_full_line_comments(&source);
        for (offset, ident) in crate_path_occurrences(&stripped) {
            if !modules.contains(&ident) {
                unmatched.push(format!(
                    "{}: byte {offset}, found {ident:?} immediately after `crate::`",
                    relative.display()
                ));
            }
        }
    }
    assert!(
        unmatched.is_empty(),
        "every `crate::` occurrence in a production file must name a known top-level module \
         immediately — these did not, and crate_path_references would have silently dropped each \
         one instead of counting it as a reference: {unmatched:?}"
    );
}

/// **RFC-074 D3, Stage 1: un-ignored by PR-074-B.** `domain` had 7
/// outgoing references, all `crate::project::ProjectId`, one per domain
/// submodule (`ownership.rs`, `audit.rs`, `terminal.rs`, `approval.rs`,
/// `transcript.rs`, `changeset.rs`, `agent.rs`). `ProjectId` now lives
/// in `domain::identity`, re-exported from `project` unchanged, and
/// those seven files import it locally (`use super::ProjectId;`) rather
/// than reaching back across the boundary that no longer exists.
#[test]
fn domain_has_zero_outgoing_cross_module_references() {
    let graph = measure_capability_graph();
    let outgoing = graph.outgoing_targets("domain");
    assert!(
        outgoing.is_empty(),
        "domain has an outgoing reference to {outgoing:?}; RFC-074 D2 requires the vocabulary \
         tier to depend on nothing"
    );
}

/// **RFC-074 D4, Stage 2: un-ignored by PR-074-B.** `runtime` had one
/// reference, `runtime -> test_support` (`launch.rs`'s own
/// `wait_for_shell_ready` importing the three `SHELL_READINESS_*`
/// constants) -- those constants are vocabulary, not behaviour, and now
/// live in `domain` alongside `ProjectId`, keeping the one shared
/// definition `tekstide`'s own separate `wait_for_shell_ready` copy
/// still reads from. The reverse reference, `test_support ->
/// runtime` (`process_is_alive` calling `is_gone_or_going`), is a
/// separate edge this assertion does not check — see
/// `test_support_reaches_no_production_module` below for that one.
#[test]
fn no_production_module_reaches_test_support() {
    let graph = measure_capability_graph();
    let reachers = graph.modules_reaching("test_support");
    assert!(
        reachers.is_empty(),
        "test_support is reached by {reachers:?}; test scaffolding must not sit inside the \
         production graph (RFC-074 D4)"
    );
}

/// **Added by PR-074-B, review 672's own recommendation made a
/// requirement, not deferred as a note.** Assertion 3 above only checks
/// one of D4's two references (`production -> test_support`); nothing
/// checked the reverse (`test_support -> production`), so B2's second
/// disposition could regress invisibly -- exactly the half-life problem
/// D1 exists to prevent. Passes today without needing an `#[ignore]`:
/// both of D4's references are gone, so `test_support` has zero
/// outgoing edges into production as of this same commit, not merely
/// zero incoming.
#[test]
fn test_support_reaches_no_production_module() {
    let graph = measure_capability_graph();
    let outgoing = graph.outgoing_targets("test_support");
    assert!(
        outgoing.is_empty(),
        "test_support has an outgoing reference to {outgoing:?}; test scaffolding must not \
         reach back into the production graph it is not part of (RFC-074 D4)"
    );
}

/// **Added by review 675 Required 1.** The gate's own method excludes
/// test code *by filename* (`tests.rs`, `*_tests.rs`, anything under a
/// `tests/` directory) -- which silently does not exclude an inline
/// `#[cfg(test)] mod tests { ... }` block sitting inside an otherwise
/// production file. Measured, not assumed: seven of these existed in
/// `tekstide-core/src` at review 675 (`recovery/instance.rs`,
/// `recovery/record.rs`, `agent/report.rs`, `runtime/terminal/
/// security.rs` and three of its own submodules), contributing 5
/// references across 4 real edges the gate was counting as production.
/// Harmless by luck that round (the same edges already existed from real
/// production code, so no bound or membership was wrong) but load-bearing
/// the moment a test added inside one of them reached `test_support` or
/// grew the SCC -- the gate would report a structural regression that
/// never happened, which is how a ratcheted bound gets raised instead of
/// trusted.
///
/// **Fixed at the root rather than gated against recurrence alone**: all
/// seven were extracted to their own `tests.rs`/`tests/` files in the
/// same commit that adds this assertion, matching the convention every
/// other test module in this crate already follows -- so this assertion
/// passes today because the violation is gone, not because it is merely
/// now detected. It stays to catch the next one.
///
/// **Round 2 (review 675 response): keyed on the property, not its one
/// observed spelling.** The first version of this assertion matched the
/// literal trimmed line `mod tests {`, which only ever catches a module
/// named exactly `tests` with no visibility modifier. Proven wrong by the
/// reviewer's own ablation: a `#[cfg(test)] mod unit_checks { ... }` block
/// -- same defect, different identifier -- passed all five assertions,
/// including this one, while still counting a reference inside it as
/// production. "Every `#[cfg(test)] mod` in this crate happens to be named
/// `tests` today" was true and is exactly the kind of fact a guard against
/// future drift cannot rest on, since the convention is what the guard
/// exists to survive. Round 2 keyed the match on `#[cfg(test)]` gating an
/// inline module body, any name, any visibility.
///
/// **Round 3: `#[cfg(test)]` itself was still one spelling among several.**
/// The reviewer's own `#[cfg(any(test, feature = "test-support"))] mod
/// support_probe { ... }` evaded round 2's attribute match entirely, and
/// is not a hypothetical -- `test_support` and `domain::shell_readiness`
/// are both gated with exactly that attribute today. Rather than widen the
/// attribute match again (a fourth spelling would only need a fifth),
/// dropped the attribute check altogether: **no production file in this
/// scan may declare a module with a body, under any name, any visibility,
/// gated by any attribute or none.** Measured first, not assumed: zero
/// such declarations exist in `tekstide-core/src` today, so the stronger
/// assertion passes by construction and removes every spelling the first
/// two rounds were chasing one at a time, rather than adding a fourth.
#[test]
fn no_production_file_declares_a_module_with_a_body() {
    let root = src_root();
    let mut violations = Vec::new();
    for file in production_files(&root) {
        let source = fs::read_to_string(&file).unwrap_or_else(|e| panic!("reading {file:?}: {e}"));
        if declares_a_module_with_a_body(&source) {
            violations.push(
                file.strip_prefix(&root)
                    .unwrap_or(&file)
                    .display()
                    .to_string(),
            );
        }
    }
    assert!(
        violations.is_empty(),
        "production file(s) with a module declared with a body ({{ ... }}) rather than pointing \
         at a separate file (;) -- a file-scanning gate counts everything in it as production, \
         whatever attribute (if any) gates the module: extract to a separate tests.rs/tests/ \
         file: {violations:?}"
    );
}

/// Whether `source` declares a module with a body (`mod <any-name> {`,
/// under any visibility) anywhere, as opposed to one that only points at a
/// separate file (`mod <any-name>;`). Attribute-blind by design (Round 3):
/// an inline module is production content to this gate's file-based scan
/// no matter what, if anything, gates it.
fn declares_a_module_with_a_body(source: &str) -> bool {
    source
        .lines()
        .any(|line| module_declaration(line.trim()) == Some(true))
}

/// Whether `trimmed` is a module declaration, and if so, whether it opens a
/// body (`Some(true)`) or merely points at a separate file (`Some(false)`).
/// `None` when `trimmed` is not a module declaration at all.
fn module_declaration(trimmed: &str) -> Option<bool> {
    let after_visibility = strip_visibility_prefix(trimmed);
    let after_mod = after_visibility.strip_prefix("mod ")?;
    let name_end = after_mod
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(after_mod.len());
    if name_end == 0 {
        return None;
    }
    Some(after_mod[name_end..].trim_start().starts_with('{'))
}

/// `trimmed` with a leading `pub`, `pub(crate)`, `pub(super)`, `pub(self)`
/// or `pub(in ...)` removed, so a module declaration's visibility does not
/// hide it from [`module_declaration`].
fn strip_visibility_prefix(trimmed: &str) -> &str {
    if let Some(after_pub) = trimmed.strip_prefix("pub") {
        if let Some(after_paren) = after_pub.trim_start().strip_prefix('(') {
            if let Some(close) = after_paren.find(')') {
                return after_paren[close + 1..].trim_start();
            }
        } else {
            return after_pub.trim_start();
        }
    }
    trimmed
}

/// **Required by RFC-077's residual decision (2026-11-16): the gate above
/// measures cycles, and taking `VerifiedCwd::from_validated` from
/// `pub(in crate::agent)` (which does not compile, `E0742`) to `pub(crate)`
/// so the type could move would delete its construction-privilege
/// guarantee while making every bound above look *better*, not worse.**
/// This is the compile-time witness the staging document requires in its
/// place (`rfcs/handoffs/077-the-capability-split/
/// staging-derived-from-measured-cut-cost.md`): exactly one production
/// call site, in the one file whose containment check immediately
/// precedes it. The `#[cfg(test)] for_test` escape hatch (`agent/
/// launch.rs`) does not call `from_validated` at all — it constructs the
/// tuple struct directly — so it does not need excluding here; measured,
/// not assumed.
#[test]
fn verified_cwd_from_validated_has_exactly_one_production_call_site() {
    const NEEDLE: &str = "VerifiedCwd::from_validated(";
    let root = src_root();
    let mut call_sites = Vec::new();
    for file in production_files(&root) {
        let source = fs::read_to_string(&file).unwrap_or_else(|e| panic!("reading {file:?}: {e}"));
        let stripped = strip_full_line_comments(&source);
        let occurrences = stripped.matches(NEEDLE).count();
        for _ in 0..occurrences {
            call_sites.push(
                file.strip_prefix(&root)
                    .unwrap_or(&file)
                    .display()
                    .to_string(),
            );
        }
    }
    assert_eq!(
        call_sites,
        vec!["agent/launch.rs".to_string()],
        "VerifiedCwd::from_validated must have exactly one production call site, in \
         agent/launch.rs's own validated-containment path — a second one anywhere, in a second \
         file or a second time in this one, is exactly the laundering \
         `approval::coordinator`'s own module doc calls structurally unreachable; found \
         {call_sites:?}"
    );
}

/// **Required alongside the assertion above, same commit, same reason --
/// kept for its message, not for its coverage (review 683 Required,
/// round 2 correction).** This pins the *spelling* `&VerifiedCwd`, not the
/// type's identity: a production commit that re-points the import --
/// `use crate::domain::SomeOtherType as VerifiedCwd;`, or a local `type
/// VerifiedCwd = PathBuf;` -- leaves this text unchanged, leaves guard 1
/// unchanged (`from_validated`'s one call site is untouched), cuts
/// `approval -> agent` for real, and improves the bound.
/// **`receive_proposal_verified_cwd_type_pin`, below, is what actually
/// stops that regression** -- see its own doc comment for the mechanism
/// and why a bare `const _: fn(...) = ...` does not compile here; this
/// comment does not restate it; a reader fixing a mismatch between this
/// assertion and that one should trust that one's doc, not this one's.
/// This assertion stays because its failure message names the exact
/// substring that moved, which a compile error does not.
#[test]
fn receive_proposal_still_requires_a_verified_cwd_not_a_raw_path() {
    let root = src_root();
    let path = root.join("approval/coordinator.rs");
    let source = fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path:?}: {e}"));
    let stripped = strip_full_line_comments(&source);
    let signature_start = stripped
        .find("pub fn receive_proposal(")
        .expect("approval::coordinator::receive_proposal must exist");
    let signature_end = stripped[signature_start..]
        .find(") -> ReceiveOutcome")
        .map(|offset| signature_start + offset)
        .expect("receive_proposal's signature must still end in `) -> ReceiveOutcome`");
    let signature = &stripped[signature_start..signature_end];
    assert!(
        signature.contains("verified_cwd: &VerifiedCwd"),
        "receive_proposal's verified-cwd parameter must stay `&VerifiedCwd`, not a raw `&Path` \
         or anything else — widening it is the other half of the regression \
         `verified_cwd_from_validated_has_exactly_one_production_call_site` cannot see by \
         itself; signature was: {signature:?}"
    );
}

/// **Review 683 Required: the actual guard.** This is an integration test,
/// so `tekstide_core::agent` is outside the graph scan and costs the
/// measurement nothing — unlike a production file naming it, which would
/// recreate the edge Stage 7 cut. Naming `VerifiedCwd` **by its full path**
/// is the point: `approval::coordinator`'s own `use crate::agent::
/// VerifiedCwd` could be re-pointed to a same-named alias or a local `type`
/// without changing a single byte of the signature text the assertion
/// above reads, but it cannot change what this path resolves to. If
/// `receive_proposal`'s `verified_cwd` parameter is ever anything other
/// than the real `agent::launch::VerifiedCwd`, this fails to compile
/// (`E0308`) — not a test that can report a failure and let the build
/// continue, which is deliberate: the property is type identity, and type
/// identity is the compiler's to hold, not a string's.
///
/// **A bare `fn(...) = ApprovalCoordinator::receive_proposal` const, the
/// first shape tried, does not compile here** — not because `impl Trait`
/// blocks it (confirmed separately, in a scratch crate, that it does not
/// in general) but because `AuditCoordinator<'a>` carries its own lifetime
/// parameter: rustc's inference for the method's hidden `CommandAuditRecorder`
/// type parameter cannot resolve against a higher-ranked fn-pointer type
/// whose own region binds a type that is itself region-parameterized
/// (`E0308`, "one type is more general than the other" — confirmed by
/// isolating the exact failure in a second scratch crate before writing
/// this comment). A plain function that *calls* `receive_proposal` with
/// named, concrete parameter types sidesteps that inference path entirely:
/// the call itself still only typechecks if every argument's type is
/// exactly right, including `verified_cwd`'s, so the property this guard
/// exists for is unchanged — only the mechanism that enforces it is.
#[allow(dead_code, clippy::too_many_arguments)]
fn receive_proposal_verified_cwd_type_pin<'a>(
    coordinator: &'a mut ApprovalCoordinator,
    project_id: ProjectId,
    agent_run_id: AgentRunId,
    verified_cwd: &'a tekstide_core::agent::VerifiedCwd,
    project_root: &'a Path,
    state_root: &'a Path,
    accepted: AcceptedProposal,
    limits: ApprovalQueueLimits,
    audit: &'a mut AuditCoordinator<'a>,
) -> ReceiveOutcome {
    coordinator.receive_proposal(
        project_id,
        agent_run_id,
        verified_cwd,
        project_root,
        state_root,
        accepted,
        limits,
        audit,
    )
}
