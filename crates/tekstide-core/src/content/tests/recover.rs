use super::{TestSandbox, root_handle, validate};
use crate::content::{
    ExternalChangeDecision, RecoveredBufferInit, TextCursor, TextDocument, TextDocumentOpenPolicy,
    TextDocumentState, TextViewport,
};
use crate::project::ProjectId;
use std::fs;

fn recovered(
    text: &str,
    recorded_modified_at: std::time::SystemTime,
    recorded_len: u64,
) -> RecoveredBufferInit {
    RecoveredBufferInit {
        text: text.to_owned(),
        cursor: TextCursor { line: 2, column: 5 },
        viewport: TextViewport {
            first_visible_line: 1,
            first_visible_column: 0,
        },
        recorded_modified_at,
        recorded_len,
    }
}

/// RFC-027 PR-027-C, D5 "unchanged": the file on disk is exactly as it was when the
/// record was written -- the recovered text (not the disk content) comes back, and the
/// document is `Dirty`, never `Clean`, since a recovered buffer is always local work the
/// file does not have.
#[test]
fn recover_with_unchanged_disk_file_restores_dirty_with_the_recorded_text() {
    let sandbox = TestSandbox::new("text-recover-unchanged");
    let project_dir = sandbox.create_dir("project");
    let file_path = sandbox.create_file_with_contents("project/notes.txt", b"saved content\n");
    let metadata = fs::metadata(&file_path).unwrap();
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let document = TextDocument::recover(
        &root,
        "notes.txt",
        TextDocumentOpenPolicy::linux_mvp(),
        recovered(
            "unsaved edit\n",
            metadata.modified().unwrap(),
            metadata.len(),
        ),
    )
    .expect("recovering against an unchanged file must not fail");

    assert_eq!(document.state(), TextDocumentState::Dirty);
    assert_eq!(document.text(), "unsaved edit\n");
    assert_eq!(document.cursor(), TextCursor { line: 2, column: 5 });
    assert_eq!(
        document.viewport(),
        TextViewport {
            first_visible_line: 1,
            first_visible_column: 0,
        }
    );
    assert!(
        !document.can_undo(),
        "D3: no undo history survives recovery"
    );
    assert!(
        !document.can_redo(),
        "D3: no redo history survives recovery"
    );
}

/// RFC-027 PR-027-C, D5 "changed": the file moved under the record (different length than
/// what was captured) -- a recovered buffer is always local work, so this is `Conflict`,
/// the same state a live dirty document's own external change already gets, never a
/// fourth state invented for the occasion.
#[test]
fn recover_with_changed_disk_file_restores_as_conflict() {
    let sandbox = TestSandbox::new("text-recover-changed");
    let project_dir = sandbox.create_dir("project");
    let file_path = sandbox.create_file_with_contents("project/notes.txt", b"saved content\n");
    let metadata = fs::metadata(&file_path).unwrap();
    let recorded_modified_at = metadata.modified().unwrap();
    // The record's own captured length no longer matches the file now on disk.
    let recorded_len = metadata.len() + 1;
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let document = TextDocument::recover(
        &root,
        "notes.txt",
        TextDocumentOpenPolicy::linux_mvp(),
        recovered("unsaved edit\n", recorded_modified_at, recorded_len),
    )
    .expect("recovering against a changed file must not fail -- it is offered as a conflict, not refused");

    assert_eq!(document.state(), TextDocumentState::Conflict);
    assert_eq!(document.text(), "unsaved edit\n");
}

/// RFC-027 PR-027-C: the real reason `recover`'s own "changed" branch must not store the
/// fresh read it just took as `last_known_snapshot` -- a *later* `refresh_external_state`
/// call (what `recover_highlighted_offer_item` runs, through
/// `ProjectSession::refresh_active_text_document`, to make the shell's own
/// `ProjectContentStatus` show the Reload control) must still see the file as changed,
/// not report `Unchanged` just because nothing moved between the two reads.
#[test]
fn recovering_a_changed_file_still_reports_changed_on_the_next_refresh() {
    let sandbox = TestSandbox::new("text-recover-changed-then-refresh");
    let project_dir = sandbox.create_dir("project");
    let file_path = sandbox.create_file_with_contents("project/notes.txt", b"saved content\n");
    let metadata = fs::metadata(&file_path).unwrap();
    let recorded_modified_at = metadata.modified().unwrap();
    let recorded_len = metadata.len() + 1;
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let mut document = TextDocument::recover(
        &root,
        "notes.txt",
        TextDocumentOpenPolicy::linux_mvp(),
        recovered("unsaved edit\n", recorded_modified_at, recorded_len),
    )
    .expect("recovering against a changed file must not fail");

    let decision = document
        .refresh_external_state(&root, TextDocumentOpenPolicy::linux_mvp())
        .expect("refreshing a recovered-as-changed document must not fail");

    assert_eq!(
        decision,
        ExternalChangeDecision::Conflict,
        "the divergence recover() found must still be visible to a later refresh, not \
         erased by recover() having just read the same disk state itself"
    );
    assert_eq!(document.state(), TextDocumentState::Conflict);
}

/// RFC-027 PR-027-C, D5 "gone": the file no longer exists at all -- still `Conflict` (the
/// same state the "changed" case gets, since a recovered buffer is always dirty), with
/// the caller's own existence check (mirroring `refresh_document_by_canonical_path`'s own
/// `ExternalDeleted` mapping) telling "changed" apart from "gone", not a new state here.
#[test]
fn recover_with_missing_disk_file_restores_as_conflict() {
    let sandbox = TestSandbox::new("text-recover-missing");
    let project_dir = sandbox.create_dir("project");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let document = TextDocument::recover(
        &root,
        "gone.txt",
        TextDocumentOpenPolicy::linux_mvp(),
        recovered(
            "unsaved edit\n",
            std::time::SystemTime::UNIX_EPOCH,
            0,
        ),
    )
    .expect("recovering against a missing file must not fail -- it is offered as a conflict, not refused");

    assert_eq!(document.state(), TextDocumentState::Conflict);
    assert_eq!(document.text(), "unsaved edit\n");
    assert!(!document.target().canonical_path.exists());
}
