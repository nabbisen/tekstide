use std::io::{self, Write};
use std::os::fd::FromRawFd;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::pty::{self, OpenPty, close_fd, sanitize_pty_output};

const SPIKE_ROWS: u16 = 24;
const SPIKE_COLS: u16 = 80;
const RESIZED_ROWS: u16 = 40;
const RESIZED_COLS: u16 = 100;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const TERMINATION_TIMEOUT: Duration = Duration::from_secs(2);
const FLOOD_TIMEOUT: Duration = Duration::from_secs(10);
const FLOOD_LINES: usize = 10_000;
const FLOOD_CAPTURE_CAP_BYTES: usize = 256 * 1024;
const LATENCY_ITERATIONS: usize = 20;

#[derive(Debug)]
pub struct SpikeReport {
    shell: String,
    cwd_category: &'static str,
    environment_policy: &'static str,
    scripted: ScriptedSmokeReport,
    resize: ResizeSmokeReport,
    termination: TerminationSmokeReport,
    output_flood: OutputFloodSmokeReport,
    latency: LatencySmokeReport,
}

impl SpikeReport {
    pub fn print(&self) {
        println!();
        println!("PTY smoke result: passed");
        println!("shell: {}", self.shell);
        println!("cwd category: {}", self.cwd_category);
        println!("environment: {}", self.environment_policy);
        println!("login shell: no");
        println!("startup files allowed: no explicit startup files requested");

        println!();
        println!("scripted input result: passed");
        println!("input sent: {}", self.scripted.input_summary);
        println!(
            "output contains marker: {}",
            self.scripted.output_contains_marker
        );
        println!("child exit status: {}", self.scripted.exit_status);
        println!("captured bytes: {}", self.scripted.captured_bytes);
        println!("--- scripted PTY output (sanitized) ---");
        print_sanitized_output(&self.scripted.rendered_output);

        println!();
        println!("resize result: passed");
        println!(
            "resize request: {}x{} -> {}x{}",
            SPIKE_ROWS, SPIKE_COLS, RESIZED_ROWS, RESIZED_COLS
        );
        println!("child observed size: {}", self.resize.observed_size);
        println!("child exit status: {}", self.resize.exit_status);
        println!("captured bytes: {}", self.resize.captured_bytes);
        println!("--- resize PTY output (sanitized) ---");
        print_sanitized_output(&self.resize.rendered_output);

        println!();
        println!("termination result: passed");
        println!("foreground child command: {}", self.termination.command);
        println!("shell pid/session leader: {}", self.termination.shell_pid);
        println!(
            "process group before signal: {}",
            self.termination.process_group_before_signal
        );
        println!("signal sequence: {}", self.termination.signal_sequence);
        println!("timeout behavior: {}", self.termination.timeout_behavior);
        println!("shell wait status: {}", self.termination.shell_wait_status);
        println!(
            "process group alive after wait: {}",
            self.termination.group_alive_after_wait
        );
        println!(
            "orphan detection result: {}",
            self.termination.orphan_detection
        );
        println!("captured bytes: {}", self.termination.captured_bytes);
        println!("--- termination PTY output (sanitized) ---");
        print_sanitized_output(&self.termination.rendered_output);

        println!();
        println!("output flood result: passed");
        println!("flood command: {}", self.output_flood.command_summary);
        println!("output amount target: {}", self.output_flood.output_amount);
        println!("temporary buffer cap: {}", self.output_flood.buffer_cap);
        println!("total bytes observed: {}", self.output_flood.total_bytes);
        println!("stored bytes: {}", self.output_flood.stored_bytes);
        println!("dropped bytes: {}", self.output_flood.dropped_bytes);
        println!("truncated: {}", self.output_flood.truncated);
        println!("truncation marker: {}", self.output_flood.truncation_marker);
        println!("memory before: {}", self.output_flood.memory_before);
        println!("memory after: {}", self.output_flood.memory_after);
        println!("recovery behavior: {}", self.output_flood.recovery_behavior);
        println!("child exit status: {}", self.output_flood.exit_status);
        println!("--- output flood sample (sanitized) ---");
        print_sanitized_output(&self.output_flood.rendered_sample);

        println!();
        println!("latency result: passed");
        println!("measurement procedure: {}", self.latency.procedure);
        println!("iterations: {}", self.latency.iterations);
        println!("terminal dimensions: {}", self.latency.terminal_dimensions);
        println!("p50: {} us", self.latency.p50_micros);
        println!("p95: {} us", self.latency.p95_micros);
        println!("worst observed: {} us", self.latency.worst_micros);
        println!("measurement limitations: {}", self.latency.limitations);
    }
}

#[derive(Debug)]
struct ScriptedSmokeReport {
    input_summary: &'static str,
    output_contains_marker: bool,
    exit_status: String,
    captured_bytes: usize,
    rendered_output: String,
}

#[derive(Debug)]
struct ResizeSmokeReport {
    observed_size: String,
    exit_status: String,
    captured_bytes: usize,
    rendered_output: String,
}

#[derive(Debug)]
struct TerminationSmokeReport {
    command: &'static str,
    shell_pid: u32,
    process_group_before_signal: String,
    signal_sequence: &'static str,
    timeout_behavior: String,
    shell_wait_status: String,
    group_alive_after_wait: bool,
    orphan_detection: &'static str,
    captured_bytes: usize,
    rendered_output: String,
}

#[derive(Debug)]
struct OutputFloodSmokeReport {
    command_summary: &'static str,
    output_amount: String,
    buffer_cap: String,
    total_bytes: usize,
    stored_bytes: usize,
    dropped_bytes: usize,
    truncated: bool,
    truncation_marker: &'static str,
    memory_before: String,
    memory_after: String,
    recovery_behavior: &'static str,
    exit_status: String,
    rendered_sample: String,
}

#[derive(Debug)]
struct LatencySmokeReport {
    procedure: &'static str,
    iterations: usize,
    terminal_dimensions: String,
    p50_micros: u128,
    p95_micros: u128,
    worst_micros: u128,
    limitations: &'static str,
}

pub fn run_all_smokes() -> Result<SpikeReport, String> {
    let shell = PathBuf::from("/bin/sh");
    if !shell.exists() {
        return Err("/bin/sh is required for the Linux PTY spike".to_string());
    }

    Ok(SpikeReport {
        shell: shell.display().to_string(),
        cwd_category: "synthetic target/tekstide-pty-spike-root directory",
        environment_policy: "minimal: TERM, LANG, LC_ALL, PATH, PS1 only",
        scripted: run_scripted_smoke(&shell)?,
        resize: run_resize_smoke(&shell)?,
        termination: run_termination_smoke(&shell)?,
        output_flood: run_output_flood_smoke(&shell)?,
        latency: run_latency_smoke(&shell)?,
    })
}

fn run_scripted_smoke(shell: &PathBuf) -> Result<ScriptedSmokeReport, String> {
    let mut pty = OpenPty::new(SPIKE_ROWS, SPIKE_COLS)?;
    let mut child = spawn_spike_shell(shell, &mut pty)?;

    let input = "printf 'tekstide-pty-ok\\n'\nexit\n";
    pty.master
        .write_all(input.as_bytes())
        .map_err(|error| format!("failed to write scripted input to PTY: {error}"))?;
    pty.master
        .flush()
        .map_err(|error| format!("failed to flush scripted PTY input: {error}"))?;

    let output = pty::read_until_child_exits(&mut pty.master, &mut child, READ_TIMEOUT)?;
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for PTY shell: {error}"))?;

    let rendered_output = sanitize_pty_output(&output);
    let output_contains_marker = rendered_output.contains("tekstide-pty-ok");

    if !output_contains_marker {
        return Err(format!(
            "PTY output did not contain expected marker; captured {} bytes",
            output.len()
        ));
    }
    if !status.success() {
        return Err(format!("PTY shell exited unsuccessfully: {status}"));
    }

    Ok(ScriptedSmokeReport {
        input_summary: "printf marker plus exit",
        output_contains_marker,
        exit_status: status.to_string(),
        captured_bytes: output.len(),
        rendered_output,
    })
}

fn run_resize_smoke(shell: &PathBuf) -> Result<ResizeSmokeReport, String> {
    let mut pty = OpenPty::new(SPIKE_ROWS, SPIKE_COLS)?;
    let mut child = spawn_spike_shell(shell, &mut pty)?;

    pty.resize(RESIZED_ROWS, RESIZED_COLS)?;

    let input = "stty size\nexit\n";
    pty.master
        .write_all(input.as_bytes())
        .map_err(|error| format!("failed to write resize probe to PTY: {error}"))?;
    pty.master
        .flush()
        .map_err(|error| format!("failed to flush resize probe input: {error}"))?;

    let output = pty::read_until_child_exits(&mut pty.master, &mut child, READ_TIMEOUT)?;
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for resize PTY shell: {error}"))?;

    let rendered_output = sanitize_pty_output(&output);
    let expected = format!("{RESIZED_ROWS} {RESIZED_COLS}");

    if !rendered_output.contains(&expected) {
        return Err(format!(
            "resize probe did not observe expected size {expected}; captured output: {rendered_output}"
        ));
    }
    if !status.success() {
        return Err(format!(
            "resize probe shell exited unsuccessfully: {status}"
        ));
    }

    Ok(ResizeSmokeReport {
        observed_size: expected,
        exit_status: status.to_string(),
        captured_bytes: output.len(),
        rendered_output,
    })
}

fn run_termination_smoke(shell: &PathBuf) -> Result<TerminationSmokeReport, String> {
    let mut pty = OpenPty::new(SPIKE_ROWS, SPIKE_COLS)?;
    let mut child = spawn_spike_shell(shell, &mut pty)?;
    let shell_pid = child.id();
    let pgid = process_group_id(shell_pid)?;
    let command = "sleep 60";

    pty.master
        .write_all(format!("{command}\n").as_bytes())
        .map_err(|error| format!("failed to write foreground child command to PTY: {error}"))?;
    pty.master
        .flush()
        .map_err(|error| format!("failed to flush foreground child command: {error}"))?;

    std::thread::sleep(Duration::from_millis(250));

    signal_process_group(pgid, libc::SIGTERM)?;
    let wait_result = wait_for_child_exit(&mut child, TERMINATION_TIMEOUT)?;

    let (shell_wait_status, timeout_behavior) = match wait_result {
        WaitResult::Exited(status) => (status, "SIGTERM ended shell before timeout".to_string()),
        WaitResult::TimedOut => {
            signal_process_group(pgid, libc::SIGKILL)?;
            let status = wait_for_child_exit(&mut child, TERMINATION_TIMEOUT)?.into_status()?;
            (
                status,
                "SIGTERM timed out; SIGKILL fallback was used".to_string(),
            )
        }
    };

    let output = pty::read_available_for(&mut pty.master, Duration::from_millis(250));
    let rendered_output = sanitize_pty_output(&output);
    let group_alive_after_wait = process_group_exists(pgid);

    if group_alive_after_wait {
        let _ = signal_process_group(pgid, libc::SIGKILL);
        return Err(format!(
            "process group {pgid} remained alive after termination smoke"
        ));
    }

    Ok(TerminationSmokeReport {
        command,
        shell_pid,
        process_group_before_signal: pgid.to_string(),
        signal_sequence: "SIGTERM to process group; SIGKILL fallback only if timeout",
        timeout_behavior,
        shell_wait_status,
        group_alive_after_wait,
        orphan_detection: "kill(-pgid, 0) returned ESRCH after shell wait",
        captured_bytes: output.len(),
        rendered_output,
    })
}

fn run_output_flood_smoke(shell: &PathBuf) -> Result<OutputFloodSmokeReport, String> {
    let mut pty = OpenPty::new(SPIKE_ROWS, SPIKE_COLS)?;
    let mut child = spawn_spike_shell(shell, &mut pty)?;
    let memory_before = current_rss_summary();

    let command = format!(
        "i=0; while [ \"$i\" -lt {FLOOD_LINES} ]; do printf 'tekstide-flood-%05d-%080d\\n' \"$i\" \"$i\"; i=$((i+1)); done\nexit\n"
    );
    pty.master
        .write_all(command.as_bytes())
        .map_err(|error| format!("failed to write output-flood command to PTY: {error}"))?;
    pty.master
        .flush()
        .map_err(|error| format!("failed to flush output-flood command: {error}"))?;

    let capped = pty::read_until_child_exits_capped(
        &mut pty.master,
        &mut child,
        FLOOD_TIMEOUT,
        FLOOD_CAPTURE_CAP_BYTES,
    )?;
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for output-flood shell: {error}"))?;
    let memory_after = current_rss_summary();

    if !status.success() {
        return Err(format!(
            "output-flood shell exited unsuccessfully: {status}"
        ));
    }
    if !capped.was_truncated() {
        return Err("output-flood capture was expected to hit the temporary cap".to_string());
    }

    Ok(OutputFloodSmokeReport {
        command_summary: "10,000-line POSIX shell printf loop",
        output_amount: format!(
            "{FLOOD_LINES} lines, {} bytes observed",
            capped.total_bytes()
        ),
        buffer_cap: format!("{} bytes", capped.cap_bytes()),
        total_bytes: capped.total_bytes(),
        stored_bytes: capped.stored_bytes(),
        dropped_bytes: capped.dropped_bytes(),
        truncated: capped.was_truncated(),
        truncation_marker: "<TRUNCATED ...>",
        memory_before,
        memory_after,
        recovery_behavior: "shell exited successfully after capped capture; subsequent latency smoke ran in same process",
        exit_status: status.to_string(),
        rendered_sample: shorten_rendered_sample(&capped.rendered_with_marker(), 4096),
    })
}

fn run_latency_smoke(shell: &PathBuf) -> Result<LatencySmokeReport, String> {
    let mut pty = OpenPty::new(SPIKE_ROWS, SPIKE_COLS)?;
    let mut child = spawn_spike_shell(shell, &mut pty)?;
    let mut samples = Vec::with_capacity(LATENCY_ITERATIONS);

    for index in 0..LATENCY_ITERATIONS {
        let marker = format!("tekstide-latency-{index:02}");
        let command = format!("printf '{marker}\\n'\n");
        let started = Instant::now();

        pty.master
            .write_all(command.as_bytes())
            .map_err(|error| format!("failed to write latency command to PTY: {error}"))?;
        pty.master
            .flush()
            .map_err(|error| format!("failed to flush latency command: {error}"))?;

        let _output = pty::read_until_contains(&mut pty.master, marker.as_bytes(), READ_TIMEOUT)?;
        samples.push(started.elapsed().as_micros());
    }

    pty.master
        .write_all(b"exit\n")
        .map_err(|error| format!("failed to write latency shell exit: {error}"))?;
    pty.master
        .flush()
        .map_err(|error| format!("failed to flush latency shell exit: {error}"))?;

    let _ = pty::read_until_child_exits(&mut pty.master, &mut child, READ_TIMEOUT)?;
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for latency shell: {error}"))?;
    if !status.success() {
        return Err(format!("latency shell exited unsuccessfully: {status}"));
    }

    samples.sort_unstable();
    Ok(LatencySmokeReport {
        procedure: "write printf marker to PTY, wait until marker is observed in PTY output, repeat",
        iterations: LATENCY_ITERATIONS,
        terminal_dimensions: format!("{SPIKE_ROWS}x{SPIKE_COLS}"),
        p50_micros: percentile(&samples, 50),
        p95_micros: percentile(&samples, 95),
        worst_micros: samples.last().copied().unwrap_or_default(),
        limitations: "single local run, scripted shell, stdout renderer only, includes shell echo/prompt overhead",
    })
}

fn spawn_spike_shell(shell: &PathBuf, pty: &mut OpenPty) -> Result<Child, String> {
    let root = synthetic_root()?;
    let stdin_fd = pty.duplicate_slave("duplicate PTY slave for stdin")?;
    let stdout_fd = pty.duplicate_slave("duplicate PTY slave for stdout")?;
    let stderr_fd = pty.duplicate_slave("duplicate PTY slave for stderr")?;
    let ctty_fd = pty.duplicate_slave("duplicate PTY slave for controlling terminal")?;

    let mut command = Command::new(shell);
    command
        .current_dir(root)
        .env_clear()
        .env("TERM", "xterm-256color")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("PATH", "/usr/bin:/bin")
        .env("PS1", "tekstide-spike$ ")
        .stdin(unsafe { Stdio::from_raw_fd(stdin_fd) })
        .stdout(unsafe { Stdio::from_raw_fd(stdout_fd) })
        .stderr(unsafe { Stdio::from_raw_fd(stderr_fd) });

    unsafe {
        command.pre_exec(move || {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::ioctl(ctty_fd, libc::TIOCSCTTY, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            libc::close(ctty_fd);
            Ok(())
        });
    }

    let child = command
        .spawn()
        .map_err(|error| format!("failed to spawn PTY shell: {error}"))?;

    close_fd(ctty_fd);
    pty.close_slave();

    Ok(child)
}

fn synthetic_root() -> Result<PathBuf, String> {
    let root = PathBuf::from("target").join("tekstide-pty-spike-root");
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("failed to create synthetic PTY test root: {error}"))?;
    Ok(root)
}

fn process_group_id(pid: u32) -> Result<i32, String> {
    let pgid = unsafe { libc::getpgid(pid as i32) };
    if pgid == -1 {
        Err(format!(
            "failed to read process group for pid {pid}: {}",
            io::Error::last_os_error()
        ))
    } else {
        Ok(pgid)
    }
}

fn signal_process_group(pgid: i32, signal: i32) -> Result<(), String> {
    let result = unsafe { libc::kill(-pgid, signal) };
    if result == -1 {
        Err(format!(
            "failed to signal process group {pgid} with signal {signal}: {}",
            io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

fn process_group_exists(pgid: i32) -> bool {
    let result = unsafe { libc::kill(-pgid, 0) };
    if result == 0 {
        return true;
    }

    io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

enum WaitResult {
    Exited(String),
    TimedOut,
}

impl WaitResult {
    fn into_status(self) -> Result<String, String> {
        match self {
            WaitResult::Exited(status) => Ok(status),
            WaitResult::TimedOut => Err("process did not exit before timeout".to_string()),
        }
    }
}

fn wait_for_child_exit(child: &mut Child, timeout: Duration) -> Result<WaitResult, String> {
    let started = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("failed to inspect child process status: {error}"))?
        {
            return Ok(WaitResult::Exited(status.to_string()));
        }

        if started.elapsed() > timeout {
            return Ok(WaitResult::TimedOut);
        }

        std::thread::sleep(Duration::from_millis(25));
    }
}

fn print_sanitized_output(output: &str) {
    print!("{output}");
    if !output.ends_with('\n') {
        println!();
    }
    println!("--- end output ---");
}

fn current_rss_summary() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find(|line| line.starts_with("VmRSS:"))
                .map(str::trim)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "VmRSS unavailable".to_string())
}

fn shorten_rendered_sample(rendered: &str, max_chars: usize) -> String {
    let mut sample: String = rendered.chars().take(max_chars).collect();
    if rendered.chars().count() > max_chars {
        sample.push_str("\n<SAMPLE SHORTENED FOR CONSOLE OUTPUT>\n");
    }
    sample
}

fn percentile(sorted_samples: &[u128], percentile: usize) -> u128 {
    if sorted_samples.is_empty() {
        return 0;
    }

    let index = ((sorted_samples.len() - 1) * percentile).div_ceil(100);
    sorted_samples[index]
}
