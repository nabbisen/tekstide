use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::content::TextDocumentState;
use crate::project::{ProjectContentStatus, ProjectId, ProjectSession};

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

/// RFC-065 D1's own boundary, found live while capturing evidence for this slice: reopening a
/// path that is already open does not reuse the existing entry -- it opens a second, fresh
/// one, and the first (however dirty) is untouched in the set. Deliberate, not a bug: reusing
/// an already-open entry by path is identity/switching work (PR-065-B's counts, PR-065-C's
/// switcher), and this slice's only job is that nothing already open is ever discarded. This
/// test locks the actual, observed boundary in rather than leaving it as something a reviewer
/// has to rediscover from a screenshot.
#[test]
fn reopening_an_already_open_path_adds_a_second_entry_rather_than_losing_the_first() {
    let root = test_root("content-reopen-same-path-keeps-first");
    std::fs::write(root.join("first.txt"), "first original\n").unwrap();
    let mut project = project_at(&root);

    project
        .open_text_document("first.txt")
        .expect("the document should open");
    project
        .replace_active_text("first original, with a real local edit\n")
        .expect("a local edit on the document");

    project
        .open_text_document("first.txt")
        .expect("reopening the same path must not be refused");

    assert_eq!(
        project.content_workspace().open_buffer_count(),
        2,
        "the reopen is a second entry, not a no-op and not a discard of the first"
    );
    assert_eq!(
        project.content_workspace().dirty_file_count(),
        1,
        "the original, edited entry is still counted dirty"
    );
    assert_eq!(
        project
            .content_workspace()
            .active_document()
            .unwrap()
            .state(),
        TextDocumentState::Clean,
        "the newly (re)opened entry reads the file fresh from disk and is clean"
    );

    cleanup_root(root);
}
