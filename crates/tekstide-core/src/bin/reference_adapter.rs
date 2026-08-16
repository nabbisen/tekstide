//! RFC-022 PR-022-B: the reference adapter.
//!
//! **This is a test-and-proof artifact, not a product feature.** It exists
//! to make RFC-021's approval protocol demonstrable end to end -- socket,
//! token, proposal, decision -- against Tekstide's own real production
//! channel and coordinator code, not a mock of either. It speaks a
//! protocol this project invented; no shipping AI CLI implements it, and
//! this program must never be presented as evidence that one does
//! (`what-the-dialog-must-not-lie-about.md` §4).
//!
//! # Usage
//!
//! ```text
//! reference_adapter [argv...]
//! ```
//!
//! Reads its capability token from `TEKSTIDE_APPROVAL_TOKEN`
//! (`tekstide_core::approval::APPROVAL_TOKEN_ENV_VAR`) and the endpoint's
//! socket path from `TEKSTIDE_APPROVAL_SOCKET_PATH`
//! (`tekstide_core::approval::APPROVAL_SOCKET_PATH_ENV_VAR`) -- the two
//! sanctioned delivery channels RFC-021/RFC-022 define (PR-022-C decided
//! the socket path also travels this way, matching the token, rather
//! than inventing a second delivery *class* for it -- see
//! `APPROVAL_SOCKET_PATH_ENV_VAR`'s own doc comment). Both are how a real
//! `spawn_adapter`-launched child learns them; this program takes neither
//! as a CLI argument, so it can be spawned identically by the production
//! path and by a test harness alike.
//!
//! `[argv...]`, if given, becomes the proposed command; if empty, this
//! program proposes a fixed default (`DEFAULT_PROPOSAL_ARGV`) instead. A
//! real AI CLI decides its own actions -- it does not take "the command
//! to propose" as a launch argument -- so the production spawn path never
//! passes any; PR-022-B's own tests pass one explicitly, to control what
//! gets proposed.
//!
//! Sends a single `CommandProposal`, waits for the resulting
//! `CommandDecision`, prints it, and exits with a code identifying what
//! happened.
//!
//! # Exit codes
//!
//! | code | meaning |
//! |---|---|
//! | 0 | decision received: `approved_once` |
//! | 1 | decision received: `rejected` |
//! | 4 | decision received: `edited_and_approved` |
//! | 2 | `TEKSTIDE_APPROVAL_TOKEN` was not set -- a defined failure, not left to whatever the socket happens to do |
//! | 3 | connect/send/read failed, or the response was malformed. This also covers a wrong or rejected token: the server closes the connection silently on an auth failure, per RFC-021's fail-closed-without-a-dialog rule, rather than replying with anything a client could distinguish from any other connection failure -- so this program cannot, and does not try to, tell "wrong token" apart from "server not listening" from the outside. |
//! | 5 | `TEKSTIDE_APPROVAL_SOCKET_PATH` was not set -- the same defined-not-guessed treatment as a missing token |
//!
//! # The wire shapes below are RFC-021's protocol, not this program's own invention
//!
//! `tekstide_core::approval::channel`'s `WireCommandProposal`/
//! `WireCommandDecision` are private to that module by design -- the
//! library's public surface is the validated `CommandProposal`/
//! `CommandDecision` types, not the wire JSON, and a `[[bin]]` target in
//! the same package is a separate crate for privacy purposes, so it
//! cannot import module-private items regardless. `ProposalWire`/
//! `DecisionWire` below mirror those private types' field names and
//! serde shapes exactly, matched by hand against `channel.rs`'s own
//! definitions. This is a client speaking a documented wire protocol, not
//! a reimplementation of the server's decoder: correctness is proven by
//! `approval::tests::reference_adapter`'s round trip, which spawns this
//! exact compiled binary against the real, unmodified
//! `ApprovalChannelEndpoint` and `ApprovalCoordinator` -- not a mock of
//! either -- and checks what this program actually printed and exited
//! with.

use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use tekstide_core::approval::{
    APPROVAL_SOCKET_PATH_ENV_VAR, APPROVAL_TOKEN_ENV_VAR, PROTOCOL_VERSION,
};

const EXIT_APPROVED: u8 = 0;
const EXIT_REJECTED: u8 = 1;
const EXIT_MISSING_TOKEN: u8 = 2;
const EXIT_PROTOCOL_FAILURE: u8 = 3;
const EXIT_EDITED_AND_APPROVED: u8 = 4;
const EXIT_MISSING_SOCKET_PATH: u8 = 5;

/// Used when no `argv` is given on the command line -- the production
/// spawn path never passes one (see this file's own module doc), so
/// this program still has something real to propose.
const DEFAULT_PROPOSAL_ARGV: &[&str] = &["echo", "tekstide-reference-adapter-default-proposal"];

/// A generous bound on how long this program will wait for a decision
/// once its proposal is sent, so a genuine regression on the server side
/// makes this program exit distinctly (`EXIT_PROTOCOL_FAILURE`) rather
/// than hang forever -- RFC-021 leaves the human-approval wait itself
/// unbounded (open question 2, still the owner's), but that is a property
/// of how long a *person* may take, not a reason for this client's own
/// socket read to have no bound at all.
const DECISION_READ_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Serialize)]
struct ProposalWire {
    protocol_version: u32,
    run_token: String,
    proposal_id: String,
    argv: Vec<String>,
    cwd: String,
    declared_intent: Option<String>,
    declared_effects: Option<String>,
}

#[derive(Deserialize)]
struct DecisionWire {
    #[allow(dead_code)]
    protocol_version: u32,
    #[allow(dead_code)]
    proposal_id: String,
    outcome: String,
    edited_argv: Option<Vec<String>>,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let argv: Vec<String> = if args.is_empty() {
        DEFAULT_PROPOSAL_ARGV
            .iter()
            .map(|entry| (*entry).to_string())
            .collect()
    } else {
        args
    };

    let token = match env::var(APPROVAL_TOKEN_ENV_VAR) {
        Ok(token) => token,
        Err(_) => {
            eprintln!(
                "{APPROVAL_TOKEN_ENV_VAR} is not set -- refusing to propose without a \
                 capability token"
            );
            return ExitCode::from(EXIT_MISSING_TOKEN);
        }
    };
    let socket_path = match env::var(APPROVAL_SOCKET_PATH_ENV_VAR) {
        Ok(socket_path) => socket_path,
        Err(_) => {
            eprintln!(
                "{APPROVAL_SOCKET_PATH_ENV_VAR} is not set -- refusing to propose without \
                 knowing where the approval channel is"
            );
            return ExitCode::from(EXIT_MISSING_SOCKET_PATH);
        }
    };

    match propose_and_await_decision(&socket_path, &token, argv) {
        Ok(outcome) => outcome.into_exit_code(),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(EXIT_PROTOCOL_FAILURE)
        }
    }
}

enum Outcome {
    ApprovedOnce,
    Rejected,
    EditedAndApproved,
}

impl Outcome {
    fn into_exit_code(self) -> ExitCode {
        ExitCode::from(match self {
            Outcome::ApprovedOnce => EXIT_APPROVED,
            Outcome::Rejected => EXIT_REJECTED,
            Outcome::EditedAndApproved => EXIT_EDITED_AND_APPROVED,
        })
    }
}

fn propose_and_await_decision(
    socket_path: &str,
    token: &str,
    argv: Vec<String>,
) -> Result<Outcome, String> {
    let mut stream = UnixStream::connect(socket_path)
        .map_err(|error| format!("connect to {socket_path} failed: {error}"))?;
    stream
        .set_read_timeout(Some(DECISION_READ_TIMEOUT))
        .map_err(|error| format!("set_read_timeout failed: {error}"))?;

    let proposal_id = format!("reference-adapter-{}", std::process::id());
    let cwd = env::current_dir()
        .map_err(|error| format!("current_dir failed: {error}"))?
        .to_string_lossy()
        .into_owned();

    let wire = ProposalWire {
        protocol_version: PROTOCOL_VERSION,
        run_token: token.to_string(),
        proposal_id,
        argv,
        cwd,
        declared_intent: None,
        declared_effects: None,
    };
    let body =
        serde_json::to_vec(&wire).map_err(|error| format!("encode proposal failed: {error}"))?;
    write_frame(&mut stream, &body)?;

    let response = read_frame(&mut stream)?;
    let decision: DecisionWire = serde_json::from_slice(&response)
        .map_err(|error| format!("decode decision failed: {error}"))?;

    match decision.outcome.as_str() {
        "approved_once" => {
            println!("DECISION: approved_once");
            Ok(Outcome::ApprovedOnce)
        }
        "rejected" => {
            println!("DECISION: rejected");
            Ok(Outcome::Rejected)
        }
        "edited_and_approved" => {
            println!(
                "DECISION: edited_and_approved argv={:?}",
                decision.edited_argv.unwrap_or_default()
            );
            Ok(Outcome::EditedAndApproved)
        }
        other => Err(format!("unrecognized decision outcome: {other}")),
    }
}

fn write_frame(stream: &mut UnixStream, bytes: &[u8]) -> Result<(), String> {
    let len =
        u32::try_from(bytes.len()).map_err(|_error| "proposal too large to frame".to_string())?;
    stream
        .write_all(&len.to_be_bytes())
        .map_err(|error| format!("write length prefix failed: {error}"))?;
    stream
        .write_all(bytes)
        .map_err(|error| format!("write proposal body failed: {error}"))
}

fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, String> {
    let mut len_bytes = [0_u8; 4];
    stream
        .read_exact(&mut len_bytes)
        .map_err(|error| format!("read decision length prefix failed: {error}"))?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    let mut buffer = vec![0_u8; len];
    stream
        .read_exact(&mut buffer)
        .map_err(|error| format!("read decision body failed: {error}"))?;
    Ok(buffer)
}
