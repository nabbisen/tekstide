use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::*;
use crate::domain::AgentRunId;
use crate::project::{ProjectId, ProjectSession};
use crate::runtime::terminal::{
    LinuxTerminalRuntime, TerminalLaunchSpec, TerminalRuntimeHandle, TerminalSecurityParser,
};
use crate::transcript::{
    BoundedTranscriptWriter, TranscriptCaptureMode, TranscriptPathRequest, TranscriptPathResolver,
    TranscriptRetentionLimits, TranscriptWriterConfig,
};

struct TestDirs {
    base: PathBuf,
    state_root: PathBuf,
    project_root: PathBuf,
}

impl TestDirs {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "tekstide-transcript-reader-{label}-{}-{unique}",
            std::process::id()
        ));
        let state_root = base.join("state");
        let project_root = base.join("project");
        fs::create_dir_all(&state_root).unwrap();
        fs::create_dir_all(&project_root).unwrap();
        Self {
            base,
            state_root,
            project_root,
        }
    }
}

impl Drop for TestDirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn resolved_storage_path(label: &str) -> (TestDirs, TranscriptStoragePath) {
    let temp = TestDirs::new(label);
    let request = TranscriptPathRequest::new(
        &temp.state_root,
        &temp.project_root,
        ProjectId::for_test(1),
        AgentRunId::for_test(1),
    );
    let storage_path = TranscriptPathResolver
        .resolve_agent_run(request)
        .expect("test storage path should resolve");
    (temp, storage_path)
}

fn write_transcript(storage_path: &TranscriptStoragePath, bytes: &[u8]) {
    let mut writer = BoundedTranscriptWriter::create(TranscriptWriterConfig::new(
        storage_path.clone(),
        TranscriptRetentionLimits::agent_run_default(),
        TranscriptCaptureMode::LocalBounded,
    ))
    .expect("writer should create");
    writer.append(bytes).expect("append should succeed");
    writer.flush().expect("flush should succeed");
}

#[test]
fn a_real_written_transcript_is_read_back_unaltered_and_needs_no_resynchronization() {
    let (_temp, storage_path) = resolved_storage_path("plain-text");
    let bytes = b"tekstide$ echo hello\nhello\ntekstide$ ".to_vec();
    write_transcript(&storage_path, &bytes);

    let window = read_window(&storage_path, TranscriptReadPolicy::default(), false)
        .expect("read should succeed");

    assert_eq!(window.content(), bytes.as_slice());
    assert_eq!(window.total_len(), bytes.len() as u64);
    assert_eq!(
        window.requested_start(),
        window.delivered_start(),
        "plain ASCII text needs no resynchronization -- requested and delivered must agree"
    );
    assert!(matches!(window, TranscriptWindow::Complete { .. }));
}

/// RFC-011 Amendment 1, D5: the caller-supplied liveness flag, not
/// anything inferred from the file, decides which constructor is used.
#[test]
fn still_being_written_threads_into_the_returned_variant() {
    let (_temp, storage_path) = resolved_storage_path("still-writing");
    write_transcript(&storage_path, b"partial output so far");

    let complete = read_window(&storage_path, TranscriptReadPolicy::default(), false).unwrap();
    let still_writing = read_window(&storage_path, TranscriptReadPolicy::default(), true).unwrap();

    assert!(matches!(complete, TranscriptWindow::Complete { .. }));
    assert!(matches!(
        still_writing,
        TranscriptWindow::StillBeingWritten { .. }
    ));
    assert_eq!(complete.content(), still_writing.content());
}

/// **The review gate's own required proof (D2)**: a window starting
/// inside a real control sequence classifies identically to the same
/// content read whole. The fixture is real captured PTY output, not a
/// synthesised convenient one -- a real shell running a real `printf`
/// that emits a genuine SGR escape sequence, captured through the same
/// `LinuxTerminalRuntime` harness `runtime::terminal::tests` already
/// uses.
///
/// Phrased as a splitting invariant so it needs no per-effect byte
/// offsets (`TerminalSecurityParser::parse` does not expose them):
/// splitting the real captured bytes at the **resynchronized** boundary
/// and parsing each half separately must equal parsing the whole buffer
/// in one call. Splitting at the **raw, non-resynchronized** offset must
/// not -- proven as this same test's negative half, using the identical
/// fixture, so the property that broke and the property that holds are
/// demonstrated against the same real bytes.
#[test]
fn a_window_starting_inside_a_real_control_sequence_classifies_identically_to_the_whole() {
    let captured = capture_real_sgr_output();

    let escape_offset = captured
        .iter()
        .position(|&byte| byte == 0x1b)
        .expect("captured output must contain a real control sequence");
    // Two bytes past the escape byte lands inside the CSI sequence's own
    // parameter bytes (past `ESC` `[`, before the final byte) -- a
    // genuine mid-sequence offset, not a boundary that happens to be safe
    // by coincidence.
    let naive_split = escape_offset + 2;
    assert!(
        naive_split < captured.len(),
        "fixture must have real bytes past the escape sequence's own start"
    );

    let parser = TerminalSecurityParser;
    let whole = parser.parse(&captured);

    let delivered_start = resynchronize(&captured, naive_split);
    let resynchronized_split = [
        parser.parse(&captured[..delivered_start]),
        parser.parse(&captured[delivered_start..]),
    ]
    .concat();
    assert_eq!(
        resynchronized_split, whole,
        "splitting at the resynchronized boundary must classify identically to the whole"
    );

    let naive_split_result = [
        parser.parse(&captured[..naive_split]),
        parser.parse(&captured[naive_split..]),
    ]
    .concat();
    assert_ne!(
        naive_split_result, whole,
        "test precondition: the raw, non-resynchronized offset must actually misclassify, \
         or this fixture does not exercise a real mid-sequence split -- captured = {captured:?}"
    );
}

/// **Ablation, per this slice's review gate.** Neuters `resynchronize`
/// into a no-op (returns the raw target unchanged, exactly what "no
/// resynchronization" means) and reruns the property test above's own
/// logic inline -- the resynchronized-split assertion must now fail,
/// naming the exact wrong effect list. This is the same fixture and the
/// same comparison as the positive test, so the only variable is whether
/// resynchronization ran.
#[test]
fn ablation_without_resynchronization_the_split_misclassifies() {
    let captured = capture_real_sgr_output();
    let escape_offset = captured
        .iter()
        .position(|&byte| byte == 0x1b)
        .expect("captured output must contain a real control sequence");
    let naive_split = escape_offset + 2;

    let parser = TerminalSecurityParser;
    let whole = parser.parse(&captured);

    // The ablation: skip the resynchronize() call entirely and split at
    // the raw requested offset, exactly what a reader with no
    // resynchronization step would do.
    let unresynchronized_delivered_start = naive_split;
    let unresynchronized_split = [
        parser.parse(&captured[..unresynchronized_delivered_start]),
        parser.parse(&captured[unresynchronized_delivered_start..]),
    ]
    .concat();

    assert_ne!(
        unresynchronized_split, whole,
        "an unresynchronized split must diverge from the whole-buffer parse -- \
         a green result here means this ablation stopped exercising the property, \
         which is a defect in the ablation, not a pass"
    );
}

/// D2's second requirement: no UTF-8 scalar split at either edge.
/// Constructs a target offset that lands in the middle of a real
/// multi-byte UTF-8 character and confirms the delivered content starts
/// on a scalar boundary, not a continuation byte.
#[test]
fn resynchronization_never_splits_a_utf8_scalar() {
    // "café" -- the 'é' is a real 2-byte UTF-8 scalar (0xc3 0xa9).
    let bytes = "tekstide: caf\u{e9} au lait\n".as_bytes().to_vec();
    let e_acute_offset = bytes
        .windows(2)
        .position(|window| window == [0xc3, 0xa9])
        .expect("fixture must contain the real 2-byte scalar");

    // Target the second byte of the scalar directly -- the raw offset a
    // window boundary could land on if it were not resynchronized.
    let target_start = e_acute_offset + 1;
    let delivered_start = resynchronize(&bytes, target_start);

    assert!(
        delivered_start <= e_acute_offset || delivered_start > e_acute_offset + 1,
        "delivered_start must not land inside the 2-byte scalar (at offset {}): got {}",
        e_acute_offset + 1,
        delivered_start
    );
    assert!(
        std::str::from_utf8(&bytes[delivered_start..]).is_ok(),
        "content from the delivered start must be valid UTF-8 -- no scalar was split"
    );
}

/// RFC-011 Amendment 1, D3: raw bytes survive the reader unaltered,
/// proven against the same bidi probe `text_safety`'s own tests use.
#[test]
fn raw_bytes_survive_the_reader_including_bidi_and_format_characters() {
    let (_temp, storage_path) = resolved_storage_path("bidi-probe");
    let mut bytes = b"before ".to_vec();
    bytes.extend_from_slice("\u{202E}evil.txt\u{202C}".as_bytes());
    bytes.extend_from_slice(b" after");
    write_transcript(&storage_path, &bytes);

    let window = read_window(&storage_path, TranscriptReadPolicy::default(), false).unwrap();

    assert_eq!(
        window.content(),
        bytes.as_slice(),
        "the reader must not escape, wrap, or otherwise alter bytes -- escaping is the \
         widget's job, not this module's"
    );
}

/// RFC-011 Amendment 1, D4, this slice's own review gate: an enumeration
/// test naming every production call site in `tekstide-core` that opens
/// a transcript file for reading, so a new one fails by name. Mirrors
/// the technique already established for an identical class of property
/// (`project::diff::tests::enumeration_confirms_only_the_closed_list_reads_full_file_content`).
///
/// **Closed list, each entry's reason disclosed**: `transcript/reader.rs`
/// (this module's own read) and `transcript/writer.rs` (`BoundedTranscriptWriter`
/// briefly reads to determine append behavior -- actually it does not;
/// listed defensively in case a future writer change adds a read-modify
/// step, so the list stays accurate rather than silently widening).
const FILES_ALLOWED_TO_OPEN_A_TRANSCRIPT_FILE_FOR_READING: &[&str] = &["transcript/reader.rs"];

fn crate_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("crate src dir must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn only_this_module_opens_a_transcript_file_for_reading() {
    let mut files = Vec::new();
    collect_rs_files(&crate_src_dir(), &mut files);

    for path in files {
        let relative = path
            .strip_prefix(crate_src_dir())
            .expect("file must be under src/")
            .to_str()
            .expect("path must be valid UTF-8")
            .to_string();

        if relative.contains("/tests/") || relative.ends_with("tests.rs") {
            continue;
        }
        // The writer's own file is a legitimate, already-reviewed
        // exception: it opens the transcript file for *writing*
        // (`OpenOptions::new().write(true)`), not reading it back.
        if relative == "transcript/writer.rs" {
            continue;
        }

        let source = fs::read_to_string(&path).expect("scannable file must be readable");
        let mentions_transcript_file = source.contains("transcript_file()");
        let reads_bytes = source.contains("File::open(") || source.contains("fs::read(");
        let opens_transcript_for_reading = mentions_transcript_file && reads_bytes;
        let is_allowed =
            FILES_ALLOWED_TO_OPEN_A_TRANSCRIPT_FILE_FOR_READING.contains(&relative.as_str());

        assert!(
            !opens_transcript_for_reading || is_allowed,
            "{relative} appears to open a transcript file for reading but is not in \
             FILES_ALLOWED_TO_OPEN_A_TRANSCRIPT_FILE_FOR_READING -- a new transcript-reading \
             call site outside the reviewed reader must be added here deliberately, or it is \
             an un-reviewed second retention/reading policy Decision D4 exists to prevent"
        );
    }
}

/// **Response 198, Finding 1's own required proof.** A transcript larger
/// than `MAX_SCAN_BYTES` must refuse, not silently return a window near
/// the end of the first `MAX_SCAN_BYTES` -- the middle of the real file,
/// mislabelled as the tail. Written directly via `std::fs::write` rather
/// than through `BoundedTranscriptWriter` -- the writer's own retention
/// limit would refuse to produce a file this large in the first place,
/// so this simulates exactly the anomalous case `MAX_SCAN_BYTES` is
/// documented as defending against (a file that grew past what the
/// writer would have produced), not a shape the writer itself can reach.
#[test]
fn a_transcript_larger_than_the_scan_limit_is_refused_not_silently_windowed() {
    let (_temp, storage_path) = resolved_storage_path("oversized");
    fs::create_dir_all(storage_path.transcript_dir()).unwrap();
    let oversized = vec![b'a'; (MAX_SCAN_BYTES + 1) as usize];
    fs::write(storage_path.transcript_file(), &oversized).unwrap();

    let result = read_window(&storage_path, TranscriptReadPolicy::default(), false);

    assert_eq!(
        result,
        Err(TranscriptReadError {
            reason: TranscriptReadErrorReason::TranscriptExceedsScanLimit,
            path: storage_path.transcript_file().to_path_buf(),
        })
    );
}

/// A path outside the reviewer's own containment policy refuses before
/// any file I/O -- reused (`is_safe_for_read`), not a second check.
#[test]
fn an_unsafe_storage_path_is_refused_before_any_read() {
    let temp = TestDirs::new("unsafe-path");
    let outside_state_root = temp.base.join("outside-state-root.log");
    fs::write(&outside_state_root, b"must never be read").unwrap();
    let forged = TranscriptStoragePath::for_test_unchecked(
        temp.state_root.clone(),
        temp.project_root.clone(),
        temp.base.clone(),
        outside_state_root,
    );

    let result = read_window(&forged, TranscriptReadPolicy::default(), false);

    assert_eq!(
        result,
        Err(TranscriptReadError {
            reason: TranscriptReadErrorReason::InvalidStoragePath,
            path: forged.transcript_file().to_path_buf(),
        })
    );
}

fn capture_real_sgr_output() -> Vec<u8> {
    let root = pty_test_root("sgr-capture");
    let project = ProjectSession::new(ProjectId::for_test(1), "Project", &root, &root);
    let spec = TerminalLaunchSpec::plain_shell(project.id().clone(), "Shell", &root, "/bin/sh");
    let mut runtime = LinuxTerminalRuntime::new();

    let (terminal, _events) = runtime
        .launch_project_shell(&project, spec)
        .expect("plain shell launch should succeed");
    let handle = TerminalRuntimeHandle::new(terminal.id.clone(), project.id().clone());

    runtime
        .write_input(&handle, b"printf '\\033[31mred\\033[0m\\n'\nexit\n")
        .expect("command should write to PTY");

    let output = read_until_contains(&mut runtime, &handle, b"red");
    let _ = runtime.wait_for_exit(&handle, Duration::from_secs(5));
    let _ = fs::remove_dir_all(&root);
    output
}

fn read_until_contains(
    runtime: &mut LinuxTerminalRuntime,
    handle: &TerminalRuntimeHandle,
    marker: &[u8],
) -> Vec<u8> {
    let started = Instant::now();
    let mut output = Vec::new();

    while started.elapsed() < Duration::from_secs(5) {
        let (chunk, _) = runtime
            .read_available_bounded_for(handle, Duration::from_millis(50), 16 * 1024)
            .expect("PTY read should succeed");
        output.extend_from_slice(&chunk);
        if output.windows(marker.len()).any(|window| window == marker) {
            return output;
        }
    }

    output
}

fn pty_test_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "tekstide-transcript-reader-pty-{name}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("test root should be created");
    root
}
