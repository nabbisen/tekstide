//! Mechanical checks over `rfcs/`, running in the ordinary test gate.
//!
//! **Why these exist as code rather than as conventions.** Both invariants
//! below were written into `ARCHITECTURE.md` as prose, by the architect,
//! after the architect had already broken them — and then broken again
//! afterwards. `ARCHITECTURE.md` states the lesson itself: *a convention
//! nobody can execute is worse than none*. These are the executable form.
//!
//! - **Status fields.** The RFC-037 five-folder migration swept for
//!   references to files, which break loudly, and never for text that
//!   *asserts a state*, which does not. Thirteen files were left claiming
//!   a state their RFC had long since left; it took the owner, the dev
//!   team, and a generalising sweep to find them all, in three separate
//!   rounds.
//! - **Relative links.** The same migration's follow-up broke a link in
//!   `RFC-036` and the architect's grep could not see it, because the
//!   reference was written `./023-...` — relative within its own folder,
//!   so it never spells the folder name a path sweep searches for. Found
//!   by resolving links, which is mechanical, rather than by guessing
//!   what string a reference contains, which is not.
//!
//! **Why they skip when `rfcs/` is absent.** The published crate does not
//! package `rfcs/`, so a consumer running `cargo test` on `tekstide` from
//! crates.io has no documents to check. Skipping is correct there and is
//! reported, never silent — an invariant that quietly passes because its
//! input is missing is the failure mode these were written against.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must resolve")
}

fn rfcs_dir() -> Option<PathBuf> {
    let dir = repo_root().join("rfcs");
    dir.is_dir().then_some(dir)
}

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            markdown_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}

/// `rfcs/{proposed,accepted,done,archive}/NNN-*.md` -> the folder holding it.
fn rfc_folder_by_number(rfcs: &Path) -> HashMap<String, String> {
    let mut located = HashMap::new();
    for folder in ["proposed", "accepted", "done", "archive"] {
        let Ok(entries) = std::fs::read_dir(rfcs.join(folder)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.len() >= 3 && name[..3].chars().all(|c| c.is_ascii_digit()) {
                located.insert(name[..3].to_owned(), folder.to_owned());
            }
        }
    }
    located
}

fn front_matter_value<'a>(source: &'a str, field: &str) -> Option<&'a str> {
    source.lines().find_map(|line| {
        let rest = line.strip_prefix(field)?.strip_prefix(':')?.trim();
        Some(rest.trim_matches('"'))
    })
}

/// The folder is the source of truth for an RFC's state (RFC-000). A
/// handoff pack's `source_rfc_status` is text asserting that same state,
/// so the two cannot be allowed to disagree.
///
/// Deliberately checks only the disagreements that are *decidable* from
/// the folder alone — a pack for an RFC in `done/` must not still claim
/// `Proposed` or `Accepted`. It does not police the exact wording, which
/// legitimately varies ("Implemented with documented limitations").
#[test]
fn every_pack_status_field_agrees_with_its_rfc_folder() {
    let Some(rfcs) = rfcs_dir() else {
        eprintln!("skipped: rfcs/ is not packaged with the published crate");
        return;
    };

    let located = rfc_folder_by_number(&rfcs);
    assert!(
        !located.is_empty(),
        "rfcs/ exists but no numbered RFC was found in any lifecycle folder"
    );

    let mut packs = Vec::new();
    markdown_files(&rfcs.join("handoffs"), &mut packs);

    let mut disagreements = Vec::new();
    for pack in &packs {
        let Ok(source) = std::fs::read_to_string(pack) else {
            continue;
        };
        let Some(status) = front_matter_value(&source, "source_rfc_status") else {
            continue;
        };
        let Some(number) = pack
            .parent()
            .and_then(|dir| dir.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| name.len() >= 3)
            .map(|name| name[..3].to_owned())
        else {
            continue;
        };
        let Some(folder) = located.get(&number) else {
            continue;
        };

        let claims_unfinished = status.contains("Proposed") || status.starts_with("Accepted");
        let stale = match folder.as_str() {
            "done" => claims_unfinished,
            "accepted" => status.contains("Proposed"),
            _ => false,
        };
        if stale {
            disagreements.push(format!(
                "  {}\n    says {status:?} but RFC-{number} is in rfcs/{folder}/",
                pack.strip_prefix(repo_root()).unwrap_or(pack).display(),
            ));
        }
    }

    assert!(
        disagreements.is_empty(),
        "a status field asserts a state the folder contradicts, and the folder wins \
         (RFC-000):\n{}\nThis is the check that would have caught all thirteen files the \
         RFC-037 migration left behind. A sweep for paths cannot find these -- they do not \
         contain a path.",
        disagreements.join("\n")
    );
}

/// Every relative link inside `rfcs/` -- to another document (`.md`) or
/// to an image (`.png`/`.jpg`/`.svg`) -- resolves to a real file.
///
/// `RFC-000` is excluded: it teaches the lifecycle using invented
/// example filenames (`./done/010-revoke-tokens.md`) that are not meant
/// to exist. That exclusion is narrow and named, not a wildcard.
#[test]
fn every_relative_link_in_the_rfc_tree_resolves() {
    let Some(rfcs) = rfcs_dir() else {
        eprintln!("skipped: rfcs/ is not packaged with the published crate");
        return;
    };

    let mut documents = Vec::new();
    markdown_files(&rfcs, &mut documents);
    assert!(!documents.is_empty(), "rfcs/ exists but holds no markdown");

    let mut broken = Vec::new();
    for document in &documents {
        if document.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .starts_with("000-rfc-lifecycle-policy")
        }) {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(document) else {
            continue;
        };

        for (index, _) in source.match_indices("](.") {
            let tail = &source[index + 2..];
            let Some(end) = tail.find(')') else { continue };
            let target = &tail[..end];
            let target = target.split('#').next().unwrap_or(target);
            let is_checked_target = [".md", ".png", ".jpg", ".svg"]
                .iter()
                .any(|ext| target.ends_with(ext));
            if !is_checked_target {
                continue;
            }
            let resolved = document
                .parent()
                .expect("a document always has a parent")
                .join(target);
            if !resolved.exists() {
                broken.push(format!(
                    "  {} -> {target}",
                    document
                        .strip_prefix(repo_root())
                        .unwrap_or(document)
                        .display(),
                ));
            }
        }
    }

    assert!(
        broken.is_empty(),
        "broken relative link(s) in the RFC tree:\n{}\nResolving links is mechanical; \
         guessing which string a reference contains is not -- a reference written \
         `./023-...` never spells its own folder, so a grep for `accepted/023` cannot \
         reach it.",
        broken.join("\n")
    );
}
