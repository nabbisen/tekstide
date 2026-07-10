mod pty;
mod smoke;

fn main() -> std::process::ExitCode {
    println!("Tekstide RFC-007 PTY feasibility harness");
    println!("scope: PR-007-E PTY feasibility, security observations, and closeout recommendation");
    println!(
        "status: spike-only; no production TerminalSession, AgentRun, transcript, or audit behavior"
    );

    match smoke::run_all_smokes() {
        Ok(report) => {
            report.print();
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("PTY smoke result: failed");
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
