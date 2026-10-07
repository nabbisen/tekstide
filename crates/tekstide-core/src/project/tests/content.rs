use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::content::{EditOperation, TextCursor, TextDocumentState};
use crate::project::{
    ProjectContentError, ProjectContentStatus, ProjectId, ProjectResourceLimits, ProjectSession,
};

fn test_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("tekstide-{name}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&root).expect("test root should be created");
    root
}

fn cleanup_root(root: PathBuf) {
    let _ = std::fs::remove_dir_all(root);
}

fn project_at(root: &std::path::Path) -> ProjectSession {
    ProjectSession::new(ProjectId::for_test(1), "Project", root, root)
}

/// **status-mapping-honesty-fixes, Fix 2's own required proof**: a real
/// file, changed on disk after a *clean* document opened it (no local
/// edits, so nothing would be lost), reports `ExternalChanged` -- not
/// `Conflict` -- once fixed. Mirrors RFC-019 PR-019-D's own "real file,
/// real external write, real operation" shape; no `SaveDecision` or
/// `TextDocumentSaveError` value is synthesised anywhere in this test.
#[test]
fn a_clean_document_saved_over_a_real_external_change_reports_external_changed_not_conflict() {
    let root = test_root("content-clean-external-change");
    std::fs::write(root.join("note.txt"), "original\n").expect("fixture file should be written");
    let mut project = project_at(&root);
    project
        .open_text_document("note.txt")
        .expect("a clean document should open");

    // No local edit -- the document stays Clean.
    std::fs::write(root.join("note.txt"), "external\n").expect("external write should succeed");

    project
        .save_active_text_document()
        .expect_err("a save over a real external change must be refused");

    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::ExternalChanged,
        "no local edits existed, so the document's own state must not claim a conflict"
    );
    assert_eq!(
        project.content_workspace().status(),
        &ProjectContentStatus::ExternalChanged,
        "the workspace-level status this slice fixes must agree with the document's own state, \
         not report the more alarming Conflict for a change that lost nothing"
    );

    cleanup_root(root);
}

/// **The genuine-conflict case must still report `Conflict`** -- the risk
/// in narrowing a status is over-narrowing it. A dirty buffer really
/// would lose a local edit on top of the real external write below.
#[test]
fn a_dirty_document_saved_over_a_real_external_change_still_reports_conflict() {
    let root = test_root("content-genuine-conflict");
    std::fs::write(root.join("note.txt"), "original\n").expect("fixture file should be written");
    let mut project = project_at(&root);
    project
        .open_text_document("note.txt")
        .expect("a clean document should open");
    project
        .replace_active_text("local edit\n")
        .expect("a local edit should be accepted");

    std::fs::write(root.join("note.txt"), "external\n").expect("external write should succeed");

    project
        .save_active_text_document()
        .expect_err("a save over a real external change must be refused");

    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::Conflict,
        "a dirty buffer really would lose the local edit -- this must still read Conflict"
    );
    assert_eq!(
        project.content_workspace().status(),
        &ProjectContentStatus::Conflict,
        "the fix must not weaken the genuine-conflict case while fixing the clean-change one"
    );

    cleanup_root(root);
}

/// RFC-026 D8: a clean open document whose file is deleted on disk reports `ExternalDeleted`, and
/// its text is kept as it was. Nothing is reloaded, and the deletion is a state the product can say.
#[test]
fn a_clean_document_whose_file_is_deleted_reports_external_deleted_and_keeps_its_text() {
    let root = test_root("content-deleted-external");
    std::fs::write(root.join("note.txt"), "original\n").expect("fixture file should be written");
    let mut project = project_at(&root);
    project
        .open_text_document("note.txt")
        .expect("a clean document should open");

    std::fs::remove_file(root.join("note.txt")).expect("external delete should succeed");
    project
        .refresh_active_text_document()
        .expect("a refresh of a deleted file is a decision, not an error");

    let document = project.content_workspace().active_document().unwrap();
    assert_eq!(
        document.text(),
        "original\n",
        "a deletion reloads nothing and keeps the text"
    );
    assert_eq!(
        project.content_workspace().status(),
        &ProjectContentStatus::ExternalDeleted,
        "a file that is gone is reported as deleted, not as changed"
    );
    cleanup_root(root);
}

/// RFC-026 D8: an unsaved edit survives an external change. A dirty document whose file changes on
/// disk is in `Conflict`, and its local edit is still there: no silent reload, nothing discarded.
#[test]
fn a_dirty_document_keeps_its_edit_when_the_file_changes_on_disk() {
    let root = test_root("content-dirty-external-keeps-edit");
    std::fs::write(root.join("note.txt"), "original\n").expect("fixture file should be written");
    let mut project = project_at(&root);
    project
        .open_text_document("note.txt")
        .expect("a clean document should open");
    project
        .replace_active_text("local original\n")
        .expect("a local edit on the open document");

    std::fs::write(root.join("note.txt"), "external\n").expect("external write should succeed");
    project
        .refresh_active_text_document()
        .expect("a refresh is a decision");

    let document = project.content_workspace().active_document().unwrap();
    assert_eq!(document.state(), TextDocumentState::Conflict);
    assert!(
        document.text().starts_with("local "),
        "the unsaved edit is kept, not replaced by the disk's version: {:?}",
        document.text()
    );
    cleanup_root(root);
}

/// RFC-026 D8, found while designing the burst measurement: a clean document that the disk changed
/// under is `ExternalChanged`, and an edit made after that is local work the disk does not have. It
/// must be a `Conflict`, so a later refresh and the save dialog both know there is something to lose.
/// It was left as `ExternalChanged`, which reads as "nothing local to lose".
#[test]
fn an_edit_made_after_an_external_change_is_a_conflict_not_a_clean_change() {
    let root = test_root("content-edit-after-external");
    std::fs::write(root.join("note.txt"), "original\n").expect("fixture file should be written");
    let mut project = project_at(&root);
    project
        .open_text_document("note.txt")
        .expect("a clean document should open");

    std::fs::write(root.join("note.txt"), "external\n").expect("external write should succeed");
    project
        .refresh_active_text_document()
        .expect("a refresh is a decision");
    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::ExternalChanged
    );

    project
        .replace_active_text("local edit\n")
        .expect("an edit on the open document");
    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::Conflict,
        "the edit is local work the disk does not have, so it is a conflict"
    );

    // A conflict is cleared by its own resolution, not by undoing the text back to what was opened.
    project
        .replace_active_text("original\n")
        .expect("the edit undone back to the opened text");
    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::Conflict,
        "a conflict is not cleared just because the text coincides with what was opened"
    );
    cleanup_root(root);
}

/// RFC-026 D8, the save path's own copy of the same fix: a blocked save against a file that is
/// gone is reported as deleted, not as changed. `refresh_active_document` already made this
/// distinction (review 463); `save_active_document` had the identical `BlockedExternalChange`
/// collapse and had not been fixed. A clean document is enough -- the file does not need local
/// edits sitting on top of it to be gone.
#[test]
fn a_save_blocked_by_a_deleted_file_reports_external_deleted_not_external_changed() {
    let root = test_root("content-save-blocked-by-deletion");
    std::fs::write(root.join("note.txt"), "original\n").expect("fixture file should be written");
    let mut project = project_at(&root);
    project
        .open_text_document("note.txt")
        .expect("a clean document should open");

    std::fs::remove_file(root.join("note.txt")).expect("external delete should succeed");

    project
        .save_active_text_document()
        .expect_err("a save against a deleted file must be refused");

    assert_eq!(
        project.content_workspace().status(),
        &ProjectContentStatus::ExternalDeleted,
        "the file is gone, not merely changed, and the save path must say so like the refresh path does"
    );
    cleanup_root(root);
}

/// RFC-065 D12: the repair's own test, written first and shown failing against today's code.
/// `Action::Open(path)` -> `open_active_project_text_document` -> `app.rs` -> `session.rs` ->
/// `content.rs`'s own `open_text_document`, which (before this slice) did
/// `self.active_document = Some(document)` with no check for `dirty`, `unsaved` or `confirm`
/// anywhere on that chain -- a user who edits a file and opens another loses the first
/// silently, undo history included. This is the one place in the product where a wrong claim
/// costs a user work they cannot get back, so the test runs against the real defect before it
/// runs against the fix (the shape RFC-026 PR-026-C used, planting the third file before any
/// code that deletes existed).
#[test]
fn opening_a_second_file_leaves_the_first_s_text_and_dirty_state_intact() {
    let root = test_root("content-open-second-keeps-first");
    std::fs::write(root.join("first.txt"), "first original\n").unwrap();
    std::fs::write(root.join("second.txt"), "second original\n").unwrap();
    let mut project = project_at(&root);

    project
        .open_text_document("first.txt")
        .expect("the first document should open");
    project
        .replace_active_text("first original, with a real local edit\n")
        .expect("a local edit on the first document");
    assert_eq!(
        project.content_workspace().dirty_file_count(),
        1,
        "test precondition: exactly one dirty document exists before the second is opened"
    );

    project
        .open_text_document("second.txt")
        .expect("the second document should open");

    assert_eq!(
        project.content_workspace().open_buffer_count(),
        2,
        "both documents must still be open -- the first was not discarded"
    );
    assert_eq!(
        project.content_workspace().dirty_file_count(),
        1,
        "the first document's unsaved edit must still be counted, even though it is no \
         longer the active one"
    );

    let first = project
        .content_workspace()
        .open_documents()
        .find(|document| {
            document.target().selected_relative_path == std::path::Path::new("first.txt")
        })
        .expect("the first document must still be in the open set");
    assert_eq!(
        first.text(),
        "first original, with a real local edit\n",
        "the first document's own edit must not have been silently discarded"
    );
    assert_eq!(first.state(), TextDocumentState::Dirty);

    cleanup_root(root);
}

/// Checklist item the first test above does not cover: "the undo history of the first
/// document survives, or the product says it did not". `replace_active_text` alone (used
/// above) never records an undo entry -- an ordinary keystroke also calls
/// `record_active_edit_operation`, so this test uses the real two-step path to plant a real
/// undo entry before opening the second file, and checks it is still there afterward.
#[test]
fn opening_a_second_file_leaves_the_first_s_undo_history_intact() {
    let root = test_root("content-open-second-keeps-first-undo");
    std::fs::write(root.join("first.txt"), "first original\n").unwrap();
    std::fs::write(root.join("second.txt"), "second original\n").unwrap();
    let mut project = project_at(&root);

    project
        .open_text_document("first.txt")
        .expect("the first document should open");
    project
        .replace_active_text("Xfirst original\n")
        .expect("a local edit on the first document");
    project
        .record_active_edit_operation(EditOperation::Insert {
            at: TextCursor::default(),
            inserted: "X".to_owned(),
        })
        .expect("the edit should be recorded for undo");

    project
        .open_text_document("second.txt")
        .expect("the second document should open");

    let first = project
        .content_workspace()
        .open_documents()
        .find(|document| {
            document.target().selected_relative_path == std::path::Path::new("first.txt")
        })
        .expect("the first document must still be in the open set");
    assert!(
        first.can_undo(),
        "the first document's undo entry must not have been lost when the second was opened"
    );
    assert!(!first.can_redo());

    cleanup_root(root);
}

/// RFC-065 history: when slice A shipped, reopening an already-open path added a **second**,
/// fresh entry rather than reusing the first -- deliberate, tested
/// (`reopening_an_already_open_path_adds_a_second_entry_rather_than_losing_the_first`, slice A's
/// own name for this test), and disclosed as identity/switching work left for later. Review 468
/// found the hazard that boundary was storing up: two entries for one path are two documents
/// that both believe they own the file, `save_active_document` saves only the active one, and
/// once PR-065-C's switcher makes the older entry reachable, saving both in either order
/// silently overwrites one with the other -- a lost update the product would create against
/// itself. The ruling moved the fix into B, before C makes the hazard reachable, with the
/// instruction to rename this test to say what it now holds rather than delete the record that
/// the old behaviour was once true and once deliberate. **This is that rename**: reopening now
/// switches to the existing entry, in place, with no disk read -- the same thing every editor a
/// user has met already does.
#[test]
fn reopening_an_already_open_path_switches_to_the_existing_entry_rather_than_adding_a_second() {
    let root = test_root("content-reopen-same-path-switches");
    std::fs::write(root.join("first.txt"), "first original\n").unwrap();
    let mut project = project_at(&root);

    project
        .open_text_document("first.txt")
        .expect("the document should open");
    project
        .replace_active_text("first original, with a real local edit\n")
        .expect("a local edit on the document");
    project
        .set_active_cursor(TextCursor { line: 0, column: 5 })
        .expect("a cursor move on the document");

    project
        .open_text_document("first.txt")
        .expect("reopening the same path must not be refused");

    assert_eq!(
        project.content_workspace().open_buffer_count(),
        1,
        "reopening an already-open path must not create a second entry"
    );
    assert_eq!(
        project.content_workspace().dirty_file_count(),
        1,
        "the entry's own unsaved edit is untouched by the reopen"
    );
    let active = project
        .content_workspace()
        .active_document()
        .expect("the entry must be active after the reopen");
    assert_eq!(
        active.text(),
        "first original, with a real local edit\n",
        "switching to the existing entry must not re-read the file from disk and discard the \
         edit"
    );
    assert_eq!(active.state(), TextDocumentState::Dirty);
    assert_eq!(
        active.cursor(),
        TextCursor { line: 0, column: 5 },
        "the existing entry's own cursor must survive the reopen, the same as any other \
         untouched field of a document that was not re-read"
    );

    cleanup_root(root);
}

/// RFC-065 D10: the close dialog needs no separate change -- `session.rs`'s own
/// `set_file_state` already feeds `close_resources.dirty_files` from `dirty_file_count`, the
/// same count this RFC's slice A made count the whole set. This is the acceptance criterion's
/// own test, not a new code path: two documents open, one dirty, and the close dialog's count
/// must say `1`, the same as `dirty_file_count` itself.
#[test]
fn the_close_dialog_counts_the_whole_open_set_through_the_existing_wiring() {
    let root = test_root("content-close-dialog-counts-set");
    std::fs::write(root.join("first.txt"), "first original\n").unwrap();
    std::fs::write(root.join("second.txt"), "second original\n").unwrap();
    let mut project = project_at(&root);

    project
        .open_text_document("first.txt")
        .expect("the first document should open");
    project
        .replace_active_text("first original, edited\n")
        .expect("a local edit on the first document");
    project
        .open_text_document("second.txt")
        .expect("the second document should open");

    assert_eq!(
        project.content_workspace().dirty_file_count(),
        1,
        "test precondition: one of the two open documents is dirty"
    );
    assert_eq!(
        project.close_resource_summary().dirty_files,
        1,
        "the close dialog's own count must agree with dirty_file_count -- D10's own point is \
         that this is the same field, not a second computation that could drift from it"
    );

    cleanup_root(root);
}

/// RFC-065 acceptance criterion: "the watcher's scope follows the open set: opening and
/// closing documents changes the watched count, counted before and after." `watch_inputs()`
/// changed from `active_document()` to `open_documents()` for this slice; this is the test
/// that proves the consequence rather than the plumbing. The project root is always watched
/// (`desired_directories` admits it unconditionally), so this counts the *document-derived*
/// directories specifically rather than the raw total.
#[test]
fn the_watched_scope_grows_as_documents_in_new_directories_open() {
    let root = test_root("content-watch-scope-grows-with-set");
    std::fs::create_dir_all(root.join("a")).unwrap();
    std::fs::create_dir_all(root.join("b")).unwrap();
    std::fs::write(root.join("a/one.txt"), "a\n").unwrap();
    std::fs::write(root.join("a/two.txt"), "a2\n").unwrap();
    std::fs::write(root.join("b/three.txt"), "b\n").unwrap();
    let mut project = project_at(&root);

    let (before, _refused) = project.watched_directories();
    assert_eq!(
        before.len(),
        1,
        "before anything is open, only the project root is watched: {before:?}"
    );

    project
        .open_text_document("a/one.txt")
        .expect("the first document should open");
    let (after_first, _refused) = project.watched_directories();
    assert_eq!(
        after_first.len(),
        2,
        "opening a document in a/ must add exactly one watched directory: {after_first:?}"
    );

    project
        .open_text_document("b/three.txt")
        .expect("the second document, in a different directory, should open");
    let (after_second, _refused) = project.watched_directories();
    assert_eq!(
        after_second.len(),
        3,
        "a second document in a new directory (b/) must add another: {after_second:?}"
    );

    project
        .open_text_document("a/two.txt")
        .expect("a third document, in an already-watched directory, should open");
    let (after_third, _refused) = project.watched_directories();
    assert_eq!(
        after_third.len(),
        3,
        "a third document in an already-watched directory (a/) must not add a fourth: \
         {after_third:?}"
    );

    cleanup_root(root);
}

/// RFC-065 D4: the open set is bounded, and the bound is stated when reached. A low limit
/// (`2`, not the real default of `20`) keeps this test from needing twenty real fixture
/// files. The already-open path is reopened first to prove the dedup switch (review 468)
/// still bypasses the bound entirely -- a path that already has a slot always belongs,
/// no matter how full the set is -- before a genuinely new path is refused.
#[test]
fn opening_past_the_bound_is_refused_and_the_refusal_is_stated() {
    let root = test_root("content-open-set-bound");
    std::fs::write(root.join("first.txt"), "first\n").unwrap();
    std::fs::write(root.join("second.txt"), "second\n").unwrap();
    std::fs::write(root.join("third.txt"), "third\n").unwrap();
    let mut project = project_at(&root);
    project.set_resource_limits(ProjectResourceLimits {
        open_document_limit: Some(2),
        ..ProjectResourceLimits::default()
    });

    project
        .open_text_document("first.txt")
        .expect("the first document should open");
    project
        .open_text_document("second.txt")
        .expect("the second document should open, reaching the limit");
    assert_eq!(project.content_workspace().open_buffer_count(), 2);

    // Reopening an already-open path must still switch, not be refused by the bound.
    project
        .open_text_document("first.txt")
        .expect("a path already in the set is never refused by the bound");
    assert_eq!(project.content_workspace().open_buffer_count(), 2);

    let error = project
        .open_text_document("third.txt")
        .expect_err("a genuinely new path past the bound must be refused");
    assert_eq!(
        error,
        ProjectContentError::OpenSetAtLimit { open: 2, limit: 2 }
    );
    assert_eq!(
        project.content_workspace().open_buffer_count(),
        2,
        "a refused open must not have joined the set"
    );
    assert_eq!(
        project.content_workspace().status().message(),
        Some("too many documents are open to open another: 2 are open, limit is 2"),
        "the refusal must be stated, not silent"
    );

    cleanup_root(root);
}
