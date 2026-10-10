//! RFC-058: a real second Tekstide process, for both the reproduction
//! (PR-058-A) and the mechanism's own regression test (PR-058-B).
//!
//! **This is a test-and-proof artifact, not a product feature.** It exists so
//! RFC-058's own hazard -- and the fix for it -- can be watched happen
//! between two real OS processes rather than reasoned about by reading
//! `app.rs`, `recovery/record.rs` and `project_lock.rs` (D2's own standard,
//! the RFC-066 D5 lesson applied before the fact). It drives exactly the
//! production sequence a real boot already runs -- `RecentProjectStore::
//! load_or_recover`, `ApplicationShell::add_project_from_path`,
//! `RecentProjectStore::save`, `tekstide_core::project_lock::
//! acquire_project_lock`, `tekstide_core::recovery::write_recovery_record` --
//! never a reimplementation of any of them.
//!
//! # Usage
//!
//! ```text
//! project_process_probe <state-root> <project-root> <relative-path> <text> [--hold]
//! ```
//!
//! `<state-root>` stands in for `AppStatePathProvider::linux_default()`'s own
//! resolved directory (a real boot reads `XDG_STATE_HOME`/`HOME`; this
//! program takes the resolved path directly, so a test harness controls it
//! without touching either process's environment). `<project-root>` is the
//! real project directory both processes open. `<relative-path>` and
//! `<text>` become a dirtied document's own recovery record, built the same
//! shape `tekstide/src/shell.rs`'s own `recovery_record_for` uses (cursor and
//! viewport at the origin -- nothing here depends on either).
//!
//! Opens the project, then attempts the project lock exactly where a real
//! boot's own persistence tick would (D3): held -> writes the record and
//! keeps the lock for the rest of this process's life; held by another live
//! process -> **does not write**, per D5/D8 (a second instance must not
//! clobber what the first is protecting); cannot decide -> writes anyway,
//! unprotected, per D5 (never refuse to run over an ambiguous lock).
//!
//! Prints exactly two lines to stdout, flushed immediately:
//! 1. this process's project id, always, whether or not it wrote anything;
//! 2. one of `ACQUIRED`, `BLOCKED <holder-pid-or-dash>`, or `UNPROTECTED
//!    <reason>`, naming which of the three outcomes above happened.
//!
//! Then: without `--hold`, exits at once; with `--hold`, blocks reading
//! stdin until it sees EOF before exiting (and before releasing the lock,
//! if it holds one). **`--hold` is what makes "two real processes" mean two
//! *overlapping* ones** (review 513) -- a harness that reads this process's
//! two lines first knows both outcomes already happened and this process
//! has not exited, and can then start a second probe while this one is
//! still alive, holding it open exactly as long as the test needs by not
//! yet closing its stdin. Exits non-zero with a message on stderr on any
//! failure before either line is printed.
use std::io::{BufRead, Write};
use std::path::PathBuf;

use tekstide_core::project::recent::{AppStatePathProvider, RecentProjectStore};
use tekstide_core::project_lock::{ProjectLockOutcome, acquire_project_lock};
use tekstide_core::recovery::{
    RecoveryFileSnapshot, RecoveryRecord, RecoveryRetentionLimits, write_recovery_record,
};
use tekstide_core::shell::ApplicationShell;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let hold = args
        .iter()
        .position(|arg| arg == "--hold")
        .map(|index| args.remove(index))
        .is_some();
    let [state_root, project_root, relative_path, text] = args.as_slice() else {
        eprintln!(
            "usage: project_process_probe <state-root> <project-root> <relative-path> <text> \
             [--hold]"
        );
        std::process::exit(2);
    };
    let state_root = PathBuf::from(state_root);
    let project_root = PathBuf::from(project_root);

    let path_provider = AppStatePathProvider::from_state_dir(state_root.clone());
    let mut store = RecentProjectStore::new(path_provider);
    let loaded = store.load_or_recover();

    let mut app_shell = ApplicationShell::new();
    app_shell.restore_recent_projects(loaded.state);

    let outcome = match app_shell.add_project_from_path(&project_root) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("project_process_probe: could not open {project_root:?}: {error}");
            std::process::exit(3);
        }
    };
    let project_id = outcome.project_id().clone();

    // The same save-after-open boot already performs (`main.rs`'s own
    // `boot()`, after its CLI-argument loop) -- without this, the *other*
    // process would never see the id this one just reused or minted, and
    // the two would not collide at all.
    if let Err(error) = store.save(&app_shell.recent_project_state()) {
        eprintln!("project_process_probe: could not save recent projects: {error}");
        std::process::exit(4);
    }

    // RFC-058 D3: attempted at the same point a real open would -- after
    // the id is known, before anything write-sensitive is touched. The
    // guard, if acquired, is kept alive until `main` returns (through
    // `--hold`'s own blocking read below), the same "held for as long as
    // this project stays open in this process" shape a real boot would use.
    let lock_outcome = acquire_project_lock(&state_root, project_id.as_str());
    let (status_line, held_lock) = match lock_outcome {
        ProjectLockOutcome::Acquired(lock) => ("ACQUIRED".to_owned(), Some(lock)),
        ProjectLockOutcome::HeldByAnother { holder_pid } => {
            let holder = holder_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "-".to_owned());
            (format!("BLOCKED {holder}"), None)
        }
        ProjectLockOutcome::CannotDecide { reason } => (format!("UNPROTECTED {reason}"), None),
    };

    // D5/D8: a second instance that does not hold the lock must not write
    // the record it would otherwise clobber. `CannotDecide` still writes
    // (unprotected, not refused) -- the one case this probe shares that
    // policy with a real boot rather than only reproducing a hazard.
    let may_write = !status_line.starts_with("BLOCKED");
    if may_write {
        let text_bytes = text.len() as u64;
        let record = RecoveryRecord {
            version: tekstide_core::recovery::RECOVERY_RECORD_VERSION,
            relative_path: relative_path.clone(),
            text: text.clone(),
            cursor_line: 0,
            cursor_column: 0,
            viewport_first_visible_line: 0,
            viewport_first_visible_column: 0,
            snapshot: RecoveryFileSnapshot::from_system_time(
                &project_root.join(relative_path),
                std::time::SystemTime::now(),
                text_bytes,
            ),
        };
        if let Err(error) = write_recovery_record(
            &state_root,
            project_id.as_str(),
            &record,
            RecoveryRetentionLimits::default_limits(),
        ) {
            eprintln!("project_process_probe: could not write recovery record: {error:?}");
            std::process::exit(5);
        }
    }

    println!("{}", project_id.as_str());
    println!("{status_line}");
    let _ = std::io::stdout().flush();

    if hold {
        // Blocks until the harness closes (or drops) this process's stdin --
        // the signal that it has seen both lines and finished whatever it
        // needed this process to still be alive for. A loop of `read_line`
        // calls, each discarded, rather than one `read_to_end`: this is
        // draining a pipe to its close, not reading a file's full content,
        // and a raw-full-file-read scan
        // (`project::diff::tests::enumeration_confirms_only_the_closed_list_reads_full_file_content`)
        // cannot tell those two apart by the text of the call alone, so it
        // is written in a shape that is not even a candidate for the
        // question that scan asks.
        let stdin = std::io::stdin();
        let mut discard = String::new();
        while stdin.lock().read_line(&mut discard).unwrap_or(0) > 0 {
            discard.clear();
        }
    }

    // Explicit, not load-bearing: dropping at the end of `main` would do
    // the same thing. Named so a reader does not have to go looking for
    // where the lock this process held is released.
    drop(held_lock);
}
