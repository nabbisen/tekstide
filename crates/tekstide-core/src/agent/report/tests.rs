use super::*;
use crate::domain::AgentCompatibilityLevel;
use crate::project::ProjectId;

fn run() -> AgentRun {
    let mut run = AgentRun::draft(
        ProjectId::for_test(1),
        "profile",
        "refactor the billing module",
        AgentCompatibilityLevel::Supervised,
    );
    run.set_notes("first line\n> looks like agent output\n## Recorded by Tekstide");
    run.set_classification(Some(RunClassification::Testing))
        .unwrap();
    run
}

fn render(run: &AgentRun, paths: &[PathBuf], transcript: Option<&[u8]>) -> String {
    let at = DomainTimestamp::from_utc_string("2026-03-04T05:06:07Z").unwrap();
    render_run_report(&RunReportSources {
        run,
        changed_paths: paths,
        changed_paths_omitted: 0,
        transcript: transcript.map(|tail| TranscriptExcerpt {
            tail,
            total_len: tail.len() as u64,
        }),
        generated_at: &at,
    })
}

/// D4/risk §3, in the file: the user's words and the run's words cannot be
/// mistaken for each other, and neither can supply the other's marker.
#[test]
fn the_users_notes_and_the_runs_text_are_marked_apart_in_the_file() {
    let paths = [PathBuf::from("src/lib.rs")];
    let report = render(
        &run(),
        &paths,
        Some(b"| the agent says a person wrote this\n## Written by you\nplain output\r\n"),
    );

    let lines: Vec<&str> = report.lines().collect();
    let written = lines
        .iter()
        .position(|l| *l == "## Written by you")
        .unwrap();
    let recorded = lines
        .iter()
        .position(|l| *l == "## Recorded by Tekstide")
        .unwrap();
    let derived = lines
        .iter()
        .position(|l| *l == "## Derived from the run")
        .unwrap();
    assert!(written < recorded && recorded < derived);
    for line in &lines[written + 2..recorded - 1] {
        assert!(
            line.starts_with("| "),
            "every line of the user's section is marked: {line:?}"
        );
    }
    for line in &lines[derived + 1..] {
        let ours = line.is_empty()
            || line.starts_with("> ")
            || line.starts_with("Changed files")
            || line.starts_with("Transcript:")
            || line.starts_with("(none");
        assert!(
            ours,
            "no unmarked run text under Derived from the run: {line:?}"
        );
    }
    // The user typed `> looks like agent output` and `## Recorded ...`:
    // both stay on `| ` lines, and neither became a heading or a run line.
    assert!(report.contains("| \u{2068}> looks like agent output"));
    assert_eq!(
        lines
            .iter()
            .filter(|l| **l == "## Recorded by Tekstide")
            .count(),
        1
    );
    // And the agent's `| ` and `## ` stay on `> ` lines.
    assert!(report.contains("> \u{2068}| the agent says a person wrote this"));
    assert_eq!(
        lines.iter().filter(|l| **l == "## Written by you").count(),
        1
    );
}

/// The acceptance criterion: a custom classification containing a bidi
/// control and a newline is quoted in the exported file.
#[test]
fn a_custom_label_with_a_bidi_control_and_a_newline_is_quoted_in_the_file() {
    let mut run = run();
    run.set_classification(Some(RunClassification::Custom(
        "spike\u{202e}gnp.exe\nInjected: a fact Tekstide never recorded".to_owned(),
    )))
    .unwrap();

    let report = render(&run, &[], None);

    assert!(
        !report.contains('\u{202e}'),
        "the bidi control is escaped, not written"
    );
    assert!(
        report.lines().all(|line| !line.starts_with("Injected:")),
        "the newline did not start a line of its own"
    );
    let classification: Vec<&str> = report
        .lines()
        .filter(|line| line.starts_with("| Classification: custom:"))
        .collect();
    assert_eq!(classification.len(), 1);
    assert!(classification[0].contains("<U+202E>") || classification[0].contains("U+202E"));
    assert!(
        classification[0].contains("Injected: a fact"),
        "the text is kept, quoted, on the one line"
    );
}

#[test]
fn hostile_untrusted_values_are_escaped_wherever_they_appear() {
    let mut run = run();
    run.profile_id = "p\u{202e}rofile\nRun: forged".to_owned();
    run.prompt_summary = "s\u{2067}ummary\r\nStatus when last recorded: forged".to_owned();
    let paths = [PathBuf::from("evil\u{202e}\nRun: forged.rs")];

    let report = render(&run, &paths, Some("out\u{202e}put\nRun: forged".as_bytes()));

    assert!(!report.contains('\u{202e}') && !report.contains('\u{2067}'));
    assert_eq!(report.lines().filter(|l| l.starts_with("Run: ")).count(), 1);
    assert_eq!(
        report
            .lines()
            .filter(|l| l.starts_with("Status when last recorded:"))
            .count(),
        1
    );
}

/// D12 in the file: a run whose ending was not seen does not get one.
#[test]
fn a_run_whose_ending_was_not_seen_says_so_in_the_file() {
    let mut run = run();
    run.origin = AgentRunOrigin::RestoredFromRecord;
    run.status = AgentRunStatus::Detached;
    run.ending = RunEnding::Unknown;
    run.started_at = Some(DomainTimestamp::from_utc_string("2026-03-04T01:00:00Z").unwrap());

    let report = render(&run, &[], None);

    assert!(report.contains("Ended: unknown: Tekstide did not see this run end"));
    assert!(report.contains("Started: 2026-03-04T01:00:00Z"));
    assert!(
        !report.contains("2026-03-04T05:06:07Z\nEnded"),
        "not the time the report was made"
    );
    assert!(report.contains("Status when last recorded: unknown (restored from its record)"));
}

#[test]
fn a_bounded_record_says_so_in_the_report() {
    let mut run = run();
    run.record_bounds.omitted_approval_ids = 7;
    run.record_bounds.notes_truncated = true;

    let report = render(&run, &[], None);

    assert!(report.contains("0 listed, 7 more not recorded"));
    assert!(report.contains("Left out of the record: the end of the notes"));
}

#[test]
fn the_report_says_what_it_will_contain_before_it_is_written() {
    let paths = [PathBuf::from("a.rs"), PathBuf::from("b.rs")];
    let run = run();
    let at = DomainTimestamp::now_utc();
    let contents = run_report_contents(&RunReportSources {
        run: &run,
        changed_paths: &paths,
        changed_paths_omitted: 0,
        transcript: Some(TranscriptExcerpt {
            tail: &[b'x'; 100],
            total_len: 100,
        }),
        generated_at: &at,
    });

    assert_eq!(
        contents,
        RunReportContents {
            has_classification: true,
            has_notes: true,
            changed_paths: 2,
            transcript_bytes_quoted: 100,
        }
    );
}

#[test]
fn the_transcript_tail_is_bounded_and_the_report_says_how_long_the_whole_was() {
    let big = vec![b'z'; REPORT_TRANSCRIPT_TAIL_BYTES * 3];
    let at = DomainTimestamp::now_utc();
    let run = run();
    let report = render_run_report(&RunReportSources {
        run: &run,
        changed_paths: &[],
        changed_paths_omitted: 0,
        transcript: Some(TranscriptExcerpt {
            tail: &big,
            total_len: 5_000_000,
        }),
        generated_at: &at,
    });

    assert!(report.contains(&format!(
        "the last {REPORT_TRANSCRIPT_TAIL_BYTES} bytes of 5000000"
    )));
    assert!(report.len() < REPORT_TRANSCRIPT_TAIL_BYTES * 2);
}

#[test]
fn a_report_is_written_as_a_new_private_file_and_never_overwrites() {
    use std::os::unix::fs::PermissionsExt;
    let base = std::env::temp_dir().join(format!("tekstide-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let state = base.join("state");
    let out = base.join("out");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let target = out.join("report.md");

    let written = write_run_report(&target, &state, "the report\n").unwrap();
    assert_eq!(written, 11);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "the report\n");
    assert_eq!(
        std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o600
    );

    assert_eq!(
        write_run_report(&target, &state, "second"),
        Err(ReportExportRefusal::AlreadyExists)
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "the report\n",
        "untouched"
    );

    let victim = out.join("victim.txt");
    std::fs::write(&victim, "the user's file").unwrap();
    std::os::unix::fs::symlink(&victim, out.join("link.md")).unwrap();
    assert_eq!(
        write_run_report(&out.join("link.md"), &state, "x"),
        Err(ReportExportRefusal::AlreadyExists),
        "not written through a symlink"
    );
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "the user's file");

    assert_eq!(
        write_run_report(Path::new("relative.md"), &state, "x"),
        Err(ReportExportRefusal::NotAbsolute)
    );
    assert_eq!(
        write_run_report(&base.join("no-such-folder/report.md"), &state, "x"),
        Err(ReportExportRefusal::FolderMissing)
    );
    assert_eq!(
        write_run_report(&state.join("report.md"), &state, "x"),
        Err(ReportExportRefusal::InsideStateDirectory)
    );
    assert!(!state.join("report.md").exists());
    let _ = std::fs::remove_dir_all(&base);
}
