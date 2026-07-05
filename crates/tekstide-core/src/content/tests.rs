use super::{
    TextCursor, TextDocument, TextDocumentEditError, TextDocumentOpenError, TextDocumentOpenPolicy,
    TextDocumentState, TextViewport, enforce_editable_size_cap,
};
use crate::project::root::{
    FileAccessBlockedReason, ProjectRootHandle, ProjectRootValidator, SymlinkPolicy,
    ValidProjectRoot,
};
use crate::project::{ProjectId, ProjectSession};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn opens_valid_utf8_file_as_clean_string_backed_document() {
    let sandbox = TestSandbox::new("text-open-valid");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/src/lib.rs", b"pub fn lib() {}\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let document = TextDocument::open(&root, "src/lib.rs", TextDocumentOpenPolicy::linux_mvp())
        .expect("valid UTF-8 file should open");

    assert_eq!(document.target().project_id, ProjectId::for_test(1));
    assert_eq!(
        document.target().selected_relative_path,
        PathBuf::from("src/lib.rs")
    );
    assert_eq!(document.text(), "pub fn lib() {}\n");
    assert_eq!(document.state(), TextDocumentState::Clean);
    assert!(!document.is_dirty());
    assert_eq!(document.cursor(), TextCursor::default());
    assert_eq!(document.viewport(), TextViewport::default());
}

#[test]
fn opens_empty_file_as_empty_clean_document() {
    let sandbox = TestSandbox::new("text-open-empty");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/empty.txt", b"");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let document = TextDocument::open(&root, "empty.txt", TextDocumentOpenPolicy::linux_mvp())
        .expect("empty UTF-8 file should open");

    assert_eq!(document.text(), "");
    assert_eq!(document.state(), TextDocumentState::Clean);
    assert!(!document.is_dirty());
}

#[test]
fn rejects_directory_targets_for_editable_open() {
    let sandbox = TestSandbox::new("text-open-directory");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_dir("project/src");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let error = TextDocument::open(&root, "src", TextDocumentOpenPolicy::linux_mvp())
        .expect_err("directory should not open as text document");

    assert!(matches!(error, TextDocumentOpenError::NotFile { .. }));
}

#[test]
fn rejects_invalid_utf8_file() {
    let sandbox = TestSandbox::new("text-open-invalid-utf8");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/bad.txt", b"bad \xFF text");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let error = TextDocument::open(&root, "bad.txt", TextDocumentOpenPolicy::linux_mvp())
        .expect_err("invalid UTF-8 should be rejected");

    assert!(matches!(error, TextDocumentOpenError::InvalidUtf8 { .. }));
}

#[test]
fn rejects_nul_containing_file_as_binary() {
    let sandbox = TestSandbox::new("text-open-nul");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/binary.txt", b"text\0more");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let error = TextDocument::open(&root, "binary.txt", TextDocumentOpenPolicy::linux_mvp())
        .expect_err("NUL-containing file should be rejected");

    assert!(matches!(error, TextDocumentOpenError::ContainsNul { .. }));
}

#[test]
fn rejects_file_above_editable_size_cap() {
    let sandbox = TestSandbox::new("text-open-large");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/large.txt", b"12345");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let policy = TextDocumentOpenPolicy {
        max_editable_bytes: 4,
    };

    let error = TextDocument::open(&root, "large.txt", policy)
        .expect_err("file above cap should be rejected");

    assert!(matches!(
        error,
        TextDocumentOpenError::TooLarge { len: 5, max: 4, .. }
    ));
}

#[test]
fn post_read_size_cap_rejects_bytes_above_cap() {
    let sandbox = TestSandbox::new("text-open-post-read-large");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"1234");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let document = TextDocument::open(
        &root,
        "file.txt",
        TextDocumentOpenPolicy {
            max_editable_bytes: 4,
        },
    )
    .expect("file at cap should open");

    let error = enforce_editable_size_cap(document.target(), 5, 4)
        .expect_err("post-read bytes above cap should be rejected");

    assert!(matches!(
        error,
        TextDocumentOpenError::TooLarge { len: 5, max: 4, .. }
    ));
}

#[test]
fn edit_transitions_document_to_dirty_without_saving() {
    let sandbox = TestSandbox::new("text-edit-dirty");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/src/lib.rs", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "src/lib.rs", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.set_cursor(TextCursor { line: 3, column: 5 });
    document.set_viewport(TextViewport {
        first_visible_line: 2,
    });
    document.replace_text("changed\n").unwrap();

    assert_eq!(document.text(), "changed\n");
    assert_eq!(document.state(), TextDocumentState::Dirty);
    assert!(document.is_dirty());
    assert_eq!(document.cursor(), TextCursor { line: 3, column: 5 });
    assert_eq!(
        document.viewport(),
        TextViewport {
            first_visible_line: 2
        }
    );
    assert_eq!(
        fs::read_to_string(project_dir.join("src/lib.rs")).unwrap(),
        "original\n",
        "PR-006-C must not save edited buffers"
    );
}

#[test]
fn replacing_text_with_same_contents_keeps_clean_document_clean() {
    let sandbox = TestSandbox::new("text-edit-same");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"same\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    document.replace_text("same\n").unwrap();

    assert_eq!(document.state(), TextDocumentState::Clean);
}

#[test]
fn replacing_text_with_nul_is_rejected_and_keeps_existing_buffer() {
    let sandbox = TestSandbox::new("text-edit-nul");
    let project_dir = sandbox.create_dir("project");
    sandbox.create_file_with_contents("project/file.txt", b"original\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));
    let mut document =
        TextDocument::open(&root, "file.txt", TextDocumentOpenPolicy::linux_mvp()).unwrap();

    let error = document
        .replace_text("changed\0text")
        .expect_err("NUL-containing replacement text should be rejected");

    assert_eq!(error, TextDocumentEditError::ContainsNul);
    assert_eq!(document.text(), "original\n");
    assert_eq!(document.state(), TextDocumentState::Clean);
}

#[test]
fn editable_open_uses_root_policy_for_cross_project_isolation() {
    let sandbox = TestSandbox::new("text-open-cross-project");
    let project_one_dir = sandbox.create_dir("project-one");
    let project_two_dir = sandbox.create_dir("project-two");
    sandbox.create_file_with_contents("project-two/file.txt", b"other\n");
    let root = root_handle(ProjectId::for_test(1), validate(&project_one_dir));
    let _other_root = root_handle(ProjectId::for_test(2), validate(&project_two_dir));

    let error = TextDocument::open(
        &root,
        "../project-two/file.txt",
        TextDocumentOpenPolicy::linux_mvp(),
    )
    .expect_err("cross-project traversal should be rejected by root policy");

    assert!(matches!(
        error,
        TextDocumentOpenError::Access(access)
            if access.reason == FileAccessBlockedReason::InvalidRelativePath
    ));
}

#[cfg(unix)]
#[test]
fn editable_open_allows_in_root_symlink_file() {
    let sandbox = TestSandbox::new("text-open-in-root-symlink");
    let project_dir = sandbox.create_dir("project");
    let target_file = sandbox.create_file_with_contents("project/src/lib.rs", b"linked\n");
    std::os::unix::fs::symlink(&target_file, project_dir.join("lib-link.rs")).unwrap();
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let document = TextDocument::open(&root, "lib-link.rs", TextDocumentOpenPolicy::linux_mvp())
        .expect("in-root symlink target should open");

    assert_eq!(document.text(), "linked\n");
}

#[cfg(unix)]
#[test]
fn editable_open_blocks_escaping_symlink_file() {
    let sandbox = TestSandbox::new("text-open-escaping-symlink");
    let project_dir = sandbox.create_dir("project");
    let outside_file = sandbox.create_file_with_contents("outside.txt", b"outside\n");
    std::os::unix::fs::symlink(&outside_file, project_dir.join("outside-link.txt")).unwrap();
    let root = root_handle(ProjectId::for_test(1), validate(&project_dir));

    let error = TextDocument::open(
        &root,
        "outside-link.txt",
        TextDocumentOpenPolicy::linux_mvp(),
    )
    .expect_err("escaping symlink should be blocked");

    assert!(matches!(
        error,
        TextDocumentOpenError::Access(access)
            if access.reason == FileAccessBlockedReason::SymlinkEscape
    ));
}

fn validate(path: &Path) -> ValidProjectRoot {
    ProjectRootValidator
        .validate(path, SymlinkPolicy::FailClosed)
        .expect("project root should validate")
}

fn root_handle(project_id: ProjectId, root: ValidProjectRoot) -> ProjectRootHandle {
    let project = ProjectSession::new(
        project_id,
        root.display_name,
        root.selected_path,
        root.canonical_path,
    );
    ProjectRootHandle::from_project_session(&project)
}

struct TestSandbox {
    root: PathBuf,
}

impl TestSandbox {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("tekstide-{name}-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self { root }
    }

    fn create_dir(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn create_file_with_contents(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
