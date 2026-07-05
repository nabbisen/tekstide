use super::{TestSandbox, root_handle, validate};
use crate::content::save::write_failed_after_temp_cleanup;
use crate::content::{
    ExternalChangeDecision, SaveDecision, TextDocument, TextDocumentOpenPolicy,
    TextDocumentSaveError, TextDocumentState,
};
use crate::project::ProjectId;
use std::fs;
use std::io;

#[test]
fn saving_dirty_document_writes_file_and_marks_document_clean() {
    let sandbox = TestSandbox::new("text-save-success");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.replace_text("changed\n").unwrap();
    let decision = document
        .save(&root, TextDocumentOpenPolicy::linux_mvp())
        .expect("dirty document should save");

    assert_eq!(decision, SaveDecision::Saved);
    assert_eq!(document.state(), TextDocumentState::Clean);
    assert!(!document.is_dirty());
    assert_eq!(
        fs::read_to_string(project_dir.join("file.txt")).unwrap(),
        "changed\n"
    );
    assert_eq!(document.last_known_snapshot().len, "changed\n".len() as u64);
}

#[test]
fn save_blocks_dirty_buffer_when_file_changed_externally() {
    let sandbox = TestSandbox::new("text-save-external-dirty");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.replace_text("tekstide edit\n").unwrap();
    fs::write(project_dir.join("file.txt"), b"external edit\n").unwrap();

    let error = document
        .save(&root, TextDocumentOpenPolicy::linux_mvp())
        .expect_err("external change should block silent overwrite");

    assert_eq!(error.decision(), SaveDecision::BlockedExternalChange);
    assert!(matches!(
        error,
        TextDocumentSaveError::ExternalChange { .. }
    ));
    assert_eq!(document.state(), TextDocumentState::Conflict);
    assert!(document.is_dirty());
    assert_eq!(document.text(), "tekstide edit\n");
    assert_eq!(
        fs::read_to_string(project_dir.join("file.txt")).unwrap(),
        "external edit\n"
    );
}

#[test]
fn clean_external_change_enters_prompt_state_without_overwriting_buffer() {
    let sandbox = TestSandbox::new("text-refresh-external-clean");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    fs::write(project_dir.join("file.txt"), b"external edit\n").unwrap();
    let decision = document
        .refresh_external_state(&root, TextDocumentOpenPolicy::linux_mvp())
        .expect("external refresh should be recoverable");

    assert_eq!(decision, ExternalChangeDecision::ExternalChanged);
    assert_eq!(document.state(), TextDocumentState::ExternalChanged);
    assert!(!document.is_dirty());
    assert_eq!(document.text(), "original\n");
}

#[test]
fn clean_deleted_file_refresh_enters_external_changed_state() {
    let sandbox = TestSandbox::new("text-refresh-deleted-clean");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    fs::remove_file(project_dir.join("file.txt")).unwrap();
    let decision = document
        .refresh_external_state(&root, TextDocumentOpenPolicy::linux_mvp())
        .expect("deleted clean file should be an external change");

    assert_eq!(decision, ExternalChangeDecision::ExternalChanged);
    assert_eq!(document.state(), TextDocumentState::ExternalChanged);
    assert!(!document.is_dirty());
    assert_eq!(document.text(), "original\n");
}

#[test]
fn clean_file_replaced_by_directory_refresh_enters_external_changed_state() {
    let sandbox = TestSandbox::new("text-refresh-directory-clean");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    fs::remove_file(project_dir.join("file.txt")).unwrap();
    fs::create_dir(project_dir.join("file.txt")).unwrap();
    let decision = document
        .refresh_external_state(&root, TextDocumentOpenPolicy::linux_mvp())
        .expect("directory replacement should be an external change");

    assert_eq!(decision, ExternalChangeDecision::ExternalChanged);
    assert_eq!(document.state(), TextDocumentState::ExternalChanged);
    assert!(!document.is_dirty());
    assert_eq!(document.text(), "original\n");
}

#[test]
fn dirty_deleted_file_save_blocks_as_external_change() {
    let sandbox = TestSandbox::new("text-save-deleted-dirty");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.replace_text("changed\n").unwrap();
    fs::remove_file(project_dir.join("file.txt")).unwrap();
    let error = document
        .save(&root, TextDocumentOpenPolicy::linux_mvp())
        .expect_err("deleted dirty file should block save as external change");

    assert_eq!(error.decision(), SaveDecision::BlockedExternalChange);
    assert!(matches!(
        error,
        TextDocumentSaveError::ExternalChange { .. }
    ));
    assert_eq!(document.state(), TextDocumentState::Conflict);
    assert!(document.is_dirty());
    assert_eq!(document.text(), "changed\n");
}

#[test]
fn dirty_file_replaced_by_directory_save_blocks_as_external_change() {
    let sandbox = TestSandbox::new("text-save-directory-dirty");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.replace_text("changed\n").unwrap();
    fs::remove_file(project_dir.join("file.txt")).unwrap();
    fs::create_dir(project_dir.join("file.txt")).unwrap();
    let error = document
        .save(&root, TextDocumentOpenPolicy::linux_mvp())
        .expect_err("directory replacement should block save as external change");

    assert_eq!(error.decision(), SaveDecision::BlockedExternalChange);
    assert!(matches!(
        error,
        TextDocumentSaveError::ExternalChange { .. }
    ));
    assert_eq!(document.state(), TextDocumentState::Conflict);
    assert!(document.is_dirty());
    assert_eq!(document.text(), "changed\n");
}

#[cfg(unix)]
#[test]
fn save_blocks_in_root_symlink_because_atomic_replacement_is_unsafe() {
    let sandbox = TestSandbox::new("text-save-in-root-symlink");
    let project_dir = sandbox.create_dir("project");
    let target_file = sandbox.create_file_with_contents("project/src/lib.rs", b"linked\n");
    std::os::unix::fs::symlink(&target_file, project_dir.join("lib-link.rs")).unwrap();
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "lib-link.rs", TextDocumentOpenPolicy::linux_mvp())
            .expect("in-root symlink opens for editing");

    document.replace_text("changed\n").unwrap();
    let error = document
        .save(&root, TextDocumentOpenPolicy::linux_mvp())
        .expect_err("symlink save should be blocked");

    assert_eq!(error.decision(), SaveDecision::BlockedUnsafeSymlink);
    assert!(matches!(error, TextDocumentSaveError::UnsafeSymlink { .. }));
    assert_eq!(document.state(), TextDocumentState::SaveError);
    assert_eq!(
        fs::read_to_string(project_dir.join("src/lib.rs")).unwrap(),
        "linked\n"
    );
}

#[cfg(unix)]
#[test]
fn save_revalidates_root_containment_before_writing() {
    let sandbox = TestSandbox::new("text-save-root-revalidate");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let outside_file = sandbox.create_file_with_contents("outside.txt", b"outside\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.replace_text("changed\n").unwrap();
    fs::remove_file(project_dir.join("file.txt")).unwrap();
    std::os::unix::fs::symlink(&outside_file, project_dir.join("file.txt")).unwrap();

    let error = document
        .save(&root, TextDocumentOpenPolicy::linux_mvp())
        .expect_err("escaping symlink at save time should be blocked");

    assert_eq!(error.decision(), SaveDecision::BlockedRootEscape);
    assert!(matches!(error, TextDocumentSaveError::RootEscape(_)));
    assert_eq!(document.state(), TextDocumentState::SaveError);
    assert_eq!(document.text(), "changed\n");
    assert_eq!(fs::read_to_string(outside_file).unwrap(), "outside\n");
}

#[cfg(unix)]
#[test]
fn write_failure_preserves_dirty_buffer_and_disk_contents() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = TestSandbox::new("text-save-write-failure");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.replace_text("changed\n").unwrap();
    fs::set_permissions(&project_dir, fs::Permissions::from_mode(0o555)).unwrap();
    let error = document
        .save(&root, TextDocumentOpenPolicy::linux_mvp())
        .expect_err("unwritable directory should fail save");
    fs::set_permissions(&project_dir, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(error.decision(), SaveDecision::WriteFailed);
    assert!(matches!(error, TextDocumentSaveError::WriteFailed { .. }));
    assert_eq!(document.state(), TextDocumentState::SaveError);
    assert!(document.is_dirty());
    assert_eq!(document.text(), "changed\n");
    assert_eq!(
        fs::read_to_string(project_dir.join("file.txt")).unwrap(),
        "original\n"
    );
}

#[test]
fn temp_cleanup_removes_best_effort_temp_file_on_write_failure() {
    let sandbox = TestSandbox::new("text-save-temp-cleanup");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();
    let temp_path = project_dir.join(".file.txt.tekstide-save-test.tmp");
    fs::write(&temp_path, b"partial").unwrap();

    let error = write_failed_after_temp_cleanup(
        document.target(),
        &temp_path,
        io::ErrorKind::PermissionDenied,
    );

    assert_eq!(error.decision(), SaveDecision::WriteFailed);
    assert!(!temp_path.exists());
}
