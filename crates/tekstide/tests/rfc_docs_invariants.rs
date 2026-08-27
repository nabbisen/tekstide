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

/// Shared by every check below that reads a status claim, wherever it
/// appears -- a handoff pack's own `source_rfc_status` front-matter
/// field, or an RFC file's own `Status:` line. One predicate, not a
/// second, differently-worded one for each site: `doc-invariants-completion.md`'s
/// own instruction for gap 1, and the reason a fix to what "stale"
/// means only has to happen once.
fn claims_unfinished(status: &str) -> bool {
    status.contains("Proposed") || status.starts_with("Accepted")
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

        let stale = match folder.as_str() {
            "done" => claims_unfinished(status),
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

/// `doc-invariants-completion.md` gap 1: the check above reads a
/// handoff pack's own `source_rfc_status` front-matter field. It never
/// reads the RFC file's own `Status:` line -- prose near the top of the
/// file (`# RFC-NNN: Title`, a blank line, then `Status: **...**...`),
/// not front matter, and the first thing a human reads. Five RFCs sat
/// in `rfcs/done/` with this line still saying `Proposed` or a bare
/// `Accepted` (039, 040 said `Proposed`; 020, 035, 038 said `Accepted`
/// with no closing update) until this slice corrected them in the same
/// commit as this check, per that handoff's own required shape. Same
/// [`claims_unfinished`] predicate as the sibling check above, not a
/// second, differently-worded one -- this one reads a different field
/// of a different file for the same disagreement.
#[test]
fn every_rfc_own_status_line_agrees_with_its_folder() {
    let Some(rfcs) = rfcs_dir() else {
        eprintln!("skipped: rfcs/ is not packaged with the published crate");
        return;
    };

    let mut disagreements = Vec::new();
    for folder in ["accepted", "done"] {
        let Ok(entries) = std::fs::read_dir(rfcs.join(folder)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.extension().is_some_and(|ext| ext == "md") {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(status_line) = source.lines().find(|line| line.starts_with("Status:")) else {
                continue;
            };
            // The RFC's own `Status:` line is prose, not a single short
            // claim the way a handoff pack's `source_rfc_status` field
            // is -- it leads with the current state, bolded, then often
            // narrates history afterward ("**Implemented and closed
            // 2026-08-26.** Proposed and accepted 2026-08-25 …", the
            // exact shape this slice's own instruction holds up as
            // correct). Applying `claims_unfinished` to the *whole*
            // line would flag that history narration for containing
            // the word "Proposed" -- reading only the bolded lead
            // claim is what makes this the same predicate as the
            // sibling check's, not a laxer one wearing the same name.
            let rest = status_line.trim_start_matches("Status:").trim();
            let lead_claim = rest
                .strip_prefix("**")
                .and_then(|after| after.split_once("**"))
                .map_or(rest, |(claim, _)| claim);

            let stale = match folder {
                "done" => claims_unfinished(lead_claim),
                "accepted" => lead_claim.contains("Proposed"),
                _ => false,
            };
            if stale {
                disagreements.push(format!(
                    "  {}\n    says {status_line:?} but sits in rfcs/{folder}/",
                    path.strip_prefix(repo_root()).unwrap_or(&path).display(),
                ));
            }
        }
    }

    assert!(
        disagreements.is_empty(),
        "an RFC's own Status: line asserts a state its folder contradicts, and the folder wins \
         (RFC-000):\n{}\nRFC-037's own argument was that this divergence misleads a reader in \
         the direction of \"this still needs deciding\" -- and this is the line a reader sees \
         first, before the handoff pack the sibling check above reads.",
        disagreements.join("\n")
    );
}

/// `doc-invariants-completion.md` gap 2: RFC-034, RFC-035 and RFC-036
/// were accepted 2026-08-18 and none appeared in `rfcs/delivery-plan.md`
/// until 2026-08-25, added retroactively when a reviewer noticed while
/// scoping something else -- the third occurrence of this exact gap,
/// not the first. `delivery-plan.md` is the file that answers "what is
/// startable work"; an accepted RFC missing from it is invisible to
/// whoever is looking for one. `proposed/` is deliberately exempt -- an
/// RFC under review has not been scheduled and should not be in the
/// queue yet.
///
/// **Matches on the RFC number in the table's first column and nothing
/// else**, per this slice's own instruction: `delivery-plan.md`'s rows
/// are prose, not a schema, and anything stricter breaks the next time
/// a row is reworded. A pipe-table row whose first cell, trimmed, is
/// not exactly three ASCII digits (a header row, a separator row, or an
/// unrelated table's own first column, e.g. `RFC-021 command approval`)
/// is not a match and is silently skipped, matching every other row
/// that is not this table at all.
///
/// **Only checks RFC-014 and above** -- the document's own header says
/// "Covers: M8 through M14," and RFC-014 is that coverage's own named
/// start ("RFC-014's substrate outcome constrains every GUI RFC after
/// it"). Confirmed empirically before adding the cutoff, not assumed:
/// every RFC 014 and above already has a row; every one of RFC-000
/// through RFC-013 (the pre-GUI, pre-M8 foundation) has none and was
/// never meant to. Checking those thirteen would not catch a real gap
/// -- it would invent one this document never claimed to close.
#[test]
fn every_accepted_or_done_rfc_has_a_delivery_plan_row() {
    let Some(rfcs) = rfcs_dir() else {
        eprintln!("skipped: rfcs/ is not packaged with the published crate");
        return;
    };

    let plan_path = rfcs.join("delivery-plan.md");
    let Ok(plan) = std::fs::read_to_string(&plan_path) else {
        eprintln!("skipped: rfcs/delivery-plan.md is not packaged with the published crate");
        return;
    };

    let scheduled: std::collections::HashSet<String> = plan
        .lines()
        .filter_map(|line| {
            let number = line.trim().strip_prefix('|')?.split('|').next()?.trim();
            (number.len() == 3 && number.chars().all(|c| c.is_ascii_digit()))
                .then(|| number.to_owned())
        })
        .collect();

    // `delivery-plan.md`'s own header: "Covers: M8 through M14
    // (`0.4.x` -> `1.0.0`)". RFC-014 is that coverage's own named start
    // ("RFC-014's substrate outcome constrains every GUI RFC after
    // it") -- confirmed empirically, not assumed: every RFC 014 and
    // above already has a row; every RFC below it (000-013, the
    // pre-GUI, pre-M8 foundation) has none and was never meant to.
    // Flagging those thirteen would not be this check catching a real
    // gap -- it would be this check inventing one the document itself
    // never claimed to close, exactly the "stricter than the document
    // actually is" failure this slice's own instruction warns against.
    const DELIVERY_PLAN_COVERAGE_STARTS_AT_RFC: u32 = 14;

    let mut missing = Vec::new();
    for folder in ["accepted", "done"] {
        let Ok(entries) = std::fs::read_dir(rfcs.join(folder)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.len() < 3 || !name[..3].chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let number = name[..3].to_owned();
            if number.parse::<u32>().unwrap_or(0) < DELIVERY_PLAN_COVERAGE_STARTS_AT_RFC {
                continue;
            }
            // RFC-037 is a process RFC (the five-folder lifecycle
            // policy itself, `rfcs/000-rfc-lifecycle-policy.md`'s own
            // successor), not "startable work" `delivery-plan.md`
            // tracks -- the document's own prose names it directly
            // ("look in rfcs/accepted/, which holds exactly those
            // (RFC-037, 2026-08-19)") without giving it a queue row,
            // the same narrow, named exception the sibling link-check
            // above gives RFC-000. Not a wildcard: a second, genuinely
            // process-only RFC would need its own explicit line here,
            // not a pattern match on "no row exists."
            if number == "037" {
                continue;
            }
            if !scheduled.contains(&number) {
                missing.push(format!("  RFC-{number} (rfcs/{folder}/{name})"));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "an RFC in accepted/ or done/ has no row in rfcs/delivery-plan.md, so it is invisible \
         to whoever is looking for startable work:\n{}\nThis is the gap RFC-034/035/036 fell \
         into for a week -- found by a reviewer scoping something else, the third occurrence, \
         not the first.",
        missing.join("\n")
    );
}

/// Every relative link inside `rfcs/` -- to another document (`.md`) or
/// to an image (`.png`/`.jpg`/`.svg`) -- resolves to a real file. Matches
/// a bare relative target (`foo.png`) as well as one written with a
/// leading `./` or `../`; a URL (`://`), an absolute path (leading `/`),
/// or a same-document anchor (leading `#`) is not a link into this tree
/// and is excluded before it ever reaches the extension check.
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

        for (index, _) in source.match_indices("](") {
            let tail = &source[index + 2..];
            let Some(end) = tail.find(')') else { continue };
            let raw_target = &tail[..end];
            // A URL, an absolute path, or a same-document anchor is not a
            // relative link into the RFC tree -- skip before even
            // stripping the anchor, so `#foo` alone is excluded here
            // rather than by falling out of the extension check below.
            if raw_target.contains("://")
                || raw_target.starts_with('/')
                || raw_target.starts_with('#')
            {
                continue;
            }
            let target = raw_target.split('#').next().unwrap_or(raw_target);
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
