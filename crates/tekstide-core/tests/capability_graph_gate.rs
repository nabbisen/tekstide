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
//! **21 top-level modules, plus `test_support` as a 22nd, special one.**
//! `lib.rs` declares 21 `pub mod`s unconditionally and one more,
//! `test_support`, gated on `any(test, feature = "test-support")` — present
//! in source either way, since this gate reads source text rather than
//! compiling, and it is exactly the node D1's third assertion is about
//! reaching. The RFC's own "21 top-level modules" counts the always-compiled
//! set; `test_support` is graphed as the 22nd because it must be, not because
//! the count is wrong.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The ratcheted bound (RFC-074 D1.1). Started at 10, PR-074-A's own
/// measured value — a ceiling, not a goal. **Lowered to 9 by PR-074-B
/// Stage 1** (`ProjectId` moved to `domain`). **Lowered to 8 by Stage 2**
/// (`test_support`'s two references to/from `runtime` both gone,
/// dropping it out of the strongly connected component entirely — not
/// merely shrinking it by one, since `test_support` is no longer reached
/// by anything in it and reaches nothing in it either). Both moves in
/// the same commit as the bound change that is their own evidence.
const MAX_LARGEST_SCC_SIZE: usize = 8;

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

/// Every occurrence of `crate::<module>` in `source` where `<module>` is one
/// of `known_modules`, as that module's name — one entry per occurrence, not
/// deduplicated (the RFC's own "references" count, as opposed to "edges").
fn crate_path_references(source: &str, known_modules: &BTreeSet<String>) -> Vec<String> {
    const PREFIX: &str = "crate::";
    let mut found = Vec::new();
    let mut search_from = 0usize;
    while let Some(at) = source[search_from..].find(PREFIX) {
        let ident_start = search_from + at + PREFIX.len();
        let ident: String = source[ident_start..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if known_modules.contains(&ident) {
            found.push(ident);
        }
        search_from = ident_start;
    }
    found
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
    assert!(
        largest.len() <= MAX_LARGEST_SCC_SIZE,
        "the largest strongly connected component has {} modules ({largest:?}); the ratcheted \
         bound is {MAX_LARGEST_SCC_SIZE} (RFC-074 D1.1) — if this grew, a stage was undone; if it \
         shrank without the bound moving with it in the same commit, the bound is now a lie",
        largest.len()
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
