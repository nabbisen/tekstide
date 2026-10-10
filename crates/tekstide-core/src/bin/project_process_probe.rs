//! RFC-058: a real second Tekstide process, for the reproduction
//! (PR-058-A), the mechanism's own regression test (PR-058-B), and the
//! open-refusal-plus-attention-request round trip (PR-058-C).
//!
//! **This is a test-and-proof artifact, not a product feature.** It exists so
//! RFC-058's own hazard -- and the fix for it -- can be watched happen
//! between two real OS processes rather than reasoned about by reading
//! `app.rs`, `recovery/record.rs` and `project_lock.rs` (D2's own standard,
//! the RFC-066 D5 lesson applied before the fact). It drives exactly the
//! production sequence a real boot already runs -- `RecentProjectStore::
//! load_or_recover`, `ApplicationShell::add_project_from_path_protected`,
//! `RecentProjectStore::save`, `tekstide_core::recovery::write_recovery_record`,
//! `tekstide_core::project_lock::attention::request_attention` -- never a
//! reimplementation of any of them.
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
//! Opens the project through the real protected path: held -> writes the
//! record and keeps the lock (and its attention listener) for the rest of
//! this process's life; held by another live process -> **the session is
//! not added at all** (D8), no write happens, and this process **sends that
//! holder a real knock** (D10) before reporting; cannot decide -> opens and
//! writes anyway, unprotected, per D5.
//!
//! Prints to stdout, flushed immediately after each line:
//! 1. this process's project id, always;
//! 2. one of `ACQUIRED`, `BLOCKED <holder-pid-or-dash> <KNOCKED|NO-KNOCK>`,
//!    or `UNPROTECTED <reason>`;
//! 3. **only if `ACQUIRED` and `--hold`**, printed after being released:
//!    `ATTENTION_RECEIVED` or `ATTENTION_NOT_RECEIVED`, reporting whether a
//!    real knock arrived on this process's own attention listener while it
//!    was held.
//!
//! Then: without `--hold`, exits at once after line 2; with `--hold`,
//! blocks reading stdin until it sees EOF before printing line 3 (if
//! `ACQUIRED`) and exiting. **`--hold` is what makes "two real processes"
//! mean two *overlapping* ones** (review 513) -- a harness that reads this
//! process's lines first knows the outcome already happened and this
//! process has not exited, and can then start a second probe while this
//! one is still alive, holding it open exactly as long as the test needs
//! by not yet closing its stdin. Exits non-zero with a message on stderr
//! on any failure before line 1 is printed.
use std::io::{BufRead, Write};
use std::path::PathBuf;

use tekstide_core::app::ProjectOpenOutcome;
use tekstide_core::project::recent::{AppStatePathProvider, RecentProjectStore};
use tekstide_core::project_lock::attention::request_attention;
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

    let outcome = match app_shell.add_project_from_path_protected(&project_root, &state_root) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("project_process_probe: could not open {project_root:?}: {error}");
            std::process::exit(3);
        }
    };

    // The same save-after-open boot already performs (`main.rs`'s own
    // `boot()`, after its CLI-argument loop) -- without this, the *other*
    // process would never see the id this one just reused or minted, and
    // the two would not collide at all. Saved regardless of outcome: even
    // a `Blocked` attempt still resolved the same id a correct re-save
    // must not disturb.
    if let Err(error) = store.save(&app_shell.recent_project_state()) {
        eprintln!("project_process_probe: could not save recent projects: {error}");
        std::process::exit(4);
    }

    let (project_id, status_line, acquired) = match outcome {
        ProjectOpenOutcome::Added(project_id) | ProjectOpenOutcome::FocusedExisting(project_id) => {
            (project_id, "ACQUIRED".to_owned(), true)
        }
        ProjectOpenOutcome::Blocked {
            project_id,
            holder_pid,
        } => {
            // D10: the knock itself. Sent from here, the caller's own
            // deliberate response to being blocked -- never bundled into
            // the open call silently.
            let knocked = request_attention(&state_root, project_id.as_str());
            let holder = holder_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "-".to_owned());
            let knock_word = if knocked { "KNOCKED" } else { "NO-KNOCK" };
            (project_id, format!("BLOCKED {holder} {knock_word}"), false)
        }
    };

    // D5/D8: a second instance that was blocked must not write the record
    // it would otherwise clobber -- `add_project_from_path_protected`
    // already refused to add the session at all, so there is nothing to
    // write for. Every other outcome (acquired, or acquired unprotected
    // under `CannotDecide`) writes, matching a real boot's own behaviour.
    if acquired {
        let text_bytes = text.len() as u64;
        let record = tekstide_core::recovery::RecoveryRecord {
            version: tekstide_core::recovery::RECOVERY_RECORD_VERSION,
            relative_path: relative_path.clone(),
            text: text.clone(),
            cursor_line: 0,
            cursor_column: 0,
            viewport_first_visible_line: 0,
            viewport_first_visible_column: 0,
            snapshot: tekstide_core::recovery::RecoveryFileSnapshot::from_system_time(
                &project_root.join(relative_path),
                std::time::SystemTime::now(),
                text_bytes,
            ),
        };
        if let Err(error) = tekstide_core::recovery::write_recovery_record(
            &state_root,
            project_id.as_str(),
            &record,
            tekstide_core::recovery::RecoveryRetentionLimits::default_limits(),
        ) {
            eprintln!("project_process_probe: could not write recovery record: {error:?}");
            std::process::exit(5);
        }
    }

    println!("{}", project_id.as_str());
    let _ = std::io::stdout().flush();
    println!("{status_line}");
    let _ = std::io::stdout().flush();

    // While held, accept real knocks on this process's own attention
    // listener (if it has one) so line 3 reports a real, observed event
    // rather than an assumption that binding implied receiving.
    let knock_received =
        acquired.then(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
    if let (true, Some(flag)) = (hold, knock_received.clone())
        && let Some(listener) = app_shell.state().try_clone_attention_listener(&project_id)
    {
        let flag = flag.clone();
        std::thread::spawn(move || {
            if listener.accept().is_ok() {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        });
    }

    if hold {
        // Blocks until the harness closes (or drops) this process's stdin --
        // the signal that it has seen the lines above and finished whatever
        // it needed this process to still be alive for. A loop of
        // `read_line` calls, each discarded, rather than one `read_to_end`:
        // this is draining a pipe to its close, not reading a file's full
        // content, and a raw-full-file-read scan
        // (`project::diff::tests::enumeration_confirms_only_the_closed_list_reads_full_file_content`)
        // cannot tell those two apart by the text of the call alone, so it
        // is written in a shape that is not even a candidate for the
        // question that scan asks.
        let stdin = std::io::stdin();
        let mut discard = String::new();
        while stdin.lock().read_line(&mut discard).unwrap_or(0) > 0 {
            discard.clear();
        }

        if let Some(flag) = knock_received {
            // A brief, bounded wait: the knock, if any, was almost
            // certainly already accepted by the time stdin closed (the
            // harness only releases this process after its own knock
            // attempt has returned), but this makes that explicit rather
            // than assumed.
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
            while !flag.load(std::sync::atomic::Ordering::SeqCst)
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            let word = if flag.load(std::sync::atomic::Ordering::SeqCst) {
                "ATTENTION_RECEIVED"
            } else {
                "ATTENTION_NOT_RECEIVED"
            };
            println!("{word}");
            let _ = std::io::stdout().flush();
        }
    }
}
