---
title: "RFC-069 PR-069-B0 — the minimum environment, measured"
rfc: "RFC-069"
rfc_file: "../../accepted/069-an-adapter-for-a-real-ai-cli.md"
source_rfc_status: "Accepted by the human owner 2026-11-15 — RFC-069 is in rfcs/accepted/"
target_milestone: "pre-1.0"
created: "2026-11-15"
---

# PR-069-B0: the minimum environment a real CLI needs, measured

No product code. No `.rs` file touched, nothing under `crates/`. This document is the whole
deliverable, per the task breakdown's own B0 scope — a proposal, not code, and no bridge code
exists until the owner answers it.

**Not re-litigated, per the brief**: `PATH` stays exactly as `spawn_adapter` already sets it
(`/usr/bin:/bin`). `resolve_executable` (`crates/tekstide-core/src/agent/launch.rs`) already
canonicalizes the profile's executable to an absolute path before any spawn — read, not re-run,
since this was established while scoping, not something this slice needed to re-derive. Every
finding below is about what the fixed, hardcoded seven-entry environment `spawn_adapter`
currently builds (`TERM`, `LANG`, `LC_ALL`, `PATH=/usr/bin:/bin`, `PS1`, plus the approval token
and socket path — `crates/tekstide-core/src/runtime/terminal/launch.rs`) is missing for a real
`claude` process, established by actually launching it, never by reasoning about what it might
need.

## A disclosure that belongs first, not buried

**The first experiment exposed fragments of the real `~/.claude/settings.json`'s own content to
my own terminal output, and I am reporting it rather than treating it as a non-event.** I launched
`claude` with `env -i` clearing everything and setting only `TERM`, `LANG`, `LC_ALL`,
`PATH=/usr/bin:/bin` — deliberately *not* setting `HOME`, expecting that to mean "no home
directory available." It does not: Node's own `os.homedir()` falls back to the system user
database (`getpwuid`) when `HOME` is absent, so the process resolved the real
`/home/nabbisen` anyway and read the real `~/.claude/settings.json` as part of its own normal
startup — printing several of that file's own permission-rule validation warnings (wildcard
patterns in `Bash(...)` allow rules) to its stderr, which I had redirected to a file and then
displayed. **This is a side effect of the CLI reading its own configuration while I observed
whether it started — the instruction's own stated carve-out — not me opening or reading the file
myself.** But I did not anticipate it, it happened, and it put real content from that file into my
own output once, in this session. I am not reproducing that content again here or anywhere else
in this document. The immediate correction — confirmed by every experiment after this one — was to
always set `HOME` explicitly to a throwaway scratch directory, never to leave it unset and assume
that means "absent." That correction is also this document's first finding: **unsetting `HOME`
does not deny the CLI access to the real one. It grants it, silently, by a fallback path a
security decision cannot afford to overlook.**

## 1. What the CLI genuinely needs, established by trying

**Method**: a Python harness (`pty.fork()`, not a plain pipe) launches the real binary
(`/home/nabbisen/.local/share/claude/versions/2.1.325`, the canonical path `readlink -f
~/.local/bin/claude` resolves to — the same install the built-in profile already targets) with a
candidate environment, in a throwaway working directory under `/tmp`, stdin closed, and a hard
deadline (4-6 seconds) after which the whole process is `SIGKILL`ed and reaped. No key was ever
sent to it. Every run was checked afterward for a leftover process under that scratch tree
(`/proc/*/exe` plus `/proc/*/cwd`, not a name-only `pgrep` — this machine runs several other real
`claude` sessions for other projects at all times, and a name match alone cannot tell them apart
from a leftover of this probe).

**Baseline, confirmed working**: `{TERM, LANG, LC_ALL, PATH=/usr/bin:/bin, HOME=<fresh empty
directory>}`. The process renders Claude Code's real first-run onboarding screen (theme picker,
version banner) inside ~1 second and sits there correctly waiting for a keystroke I never send,
until the deadline kills it. It wrote its own state under the scoped `HOME` I gave it —
`.claude.json`, `.claude/cache/`, `.claude/sessions/`, `.claude/backups/` all appeared there,
confirmed by `find`, not by reading what is inside them — and nothing was observed touching the
real `~/.claude` in this run.

**Removing each of `TERM`, `LANG`, `LC_ALL`, and `PATH` individually, and all four at once
(`HOME` alone, nothing else), produced the identical screen.** None of the four blocks reaching a
working session at this stage. This does not mean they are useless in a real, running session
(`PATH` in particular is almost certainly needed once the CLI spawns its own child tools) — it
means none of them is a *startup* requirement the way `HOME` is, so none of them is this
document's finding.

**`HOME` is the one new requirement, and it is required in a specific, narrow sense: it must be
set, and it must be non-empty.** Three ways of not doing that, all tried, all different:

- **Unset entirely**: falls back to the real `/home/nabbisen` (the disclosure above).
- **Set to the empty string (`HOME=""`)**: behaves the same as unset — it reached a workspace-trust
  prompt for the scratch directory rather than fresh onboarding, which only makes sense if it had
  resolved the real, already-configured home and recognized the theme was already set there. I did
  not need to reproduce any of that file's content to see this; the screen itself (a generic trust
  prompt naming the scratch path) was the evidence.
- **Set to a fresh, non-empty, scoped path**: works cleanly, isolates every byte of state under
  that path, confirmed above.

**Sources**: every claim in this section is this session's own command output, not a reference
page. The `pty.fork()` harness, the exact environments tried, and the exact screens reached are
reproducible from the commands recorded in this document.

## 2. What granting `HOME` exposes

In RFC-004 §7's own vocabulary: the current `spawn_adapter` environment is "clean/minimal."
Granting the real `HOME` would not be a "profile-defined environment addition" in the sense that
section anticipates — it is a single variable that is, by itself, most of the way to "inherit
host environment," because of what conventionally lives under it for a CLI shaped like this one.
Concretely, for the measured `claude` 2.1.325 install, confirmed structurally by what this
session's own scoped-`HOME` experiments wrote there (not by reading the real one's contents):

- **The CLI's own configuration and credentials.** `~/.claude.json` and `~/.claude/` hold, per
  this product's own PR-069-A measurement (`upstream-interface-measured.md`, read rather than
  re-derived here) and this session's own observed directory listing: settings (including any
  `hooks` entries a user has configured — themselves arbitrary shell commands that would run with
  the real user's own permissions), session and transcript records, and whatever authentication
  state the CLI persists once a user has logged in (this session never reached that state, so
  *what shape it takes on disk* is unestablished — see §4 — but *that something would be there*
  is not in question: the CLI has to remember being logged in somehow, and `HOME` is where it
  keeps everything else).
- **Every other program that reads `$HOME` as a matter of course** — shell rc files, git's own
  global config, any other tool `claude` itself shells out to while it operates (per
  `upstream-interface-measured.md` item 4's own finding, `PreToolUse` does not see every call,
  and this session did not test what a running `claude` actually executes). Granting the real
  `HOME` is not scoped to "what `claude` itself needs"; it is "everything this user's account
  keeps under their home directory," available to a third-party process this project did not
  write.
- **This is exactly the question D1's premise rests on, now with a concrete shape**: a bridge
  that supervises command approval by handing the supervised program the keys to the supervisor's
  own account is a strange shape for a security feature to take, and the owner is the one who
  gets to decide whether that trade is worth what RFC-069 D1 asks for.

## 3. A narrower form exists, and it already works — confirmed, not proposed on faith

**A scoped, throwaway directory, created fresh per `AgentRun`, passed as `HOME`, is the narrower
form — and §1 already showed it working, not merely plausible.** It is not a half-measure that
costs something unstated: the CLI treated it as a completely ordinary home directory, wrote its
own state there, and rendered the identical first-run UI a brand-new install would show anywhere
else. The cost, named rather than hidden:

- **Every `AgentRun` starts from onboarding again** (theme choice, and whatever the CLI's own
  trust/auth flow asks beyond the screen this session reached), unless the bridge seeds a fresh
  scoped `HOME` from a *template* it controls — a `.claude.json`/`.claude/settings.json` this
  project writes itself, naming only what the bridge needs on, never copied from the real one.
  Whether that template can include working authentication without reading the real `HOME`'s own
  credential state is exactly the open question in §4.
- **Nothing about the scoped directory needs to resemble the real one.** `/tmp/<run-id>/home` or
  an equivalent under this project's own state root works precisely because the CLI does not
  check whether `HOME` looks like a real user's home — it only needs somewhere writable, which
  this session confirmed by using `/tmp` itself.

This is the proposal: **a fresh, scoped, bridge-owned directory as `HOME` for every `AgentRun`,
never the real one.** It is cheaper in exposure than the real `HOME` by the entire list in §2, at
the cost of re-onboarding per run unless the bridge supplies its own template — a real cost, named
rather than assumed away, and smaller than "grant the real `HOME`" by every measure this session
could observe.

## 4. What this session could not establish, and why — listed, not guessed

- **Whether an authenticated, working session is reachable from a scoped `HOME` without reading
  the real one's credential state.** Establishing this means progressing past the first-run
  onboarding screen this session deliberately stopped at — the instruction's own method was
  "launch, observe, capture, stop," not "launch and operate," and getting further requires at
  least one keystroke even if it never reaches the model. I chose not to take that step rather
  than stretch the boundary past what was asked. **This is the single largest open question B1
  would need answered before it could do anything, and it is not answered here.**
- **Whether `ANTHROPIC_API_KEY` (or an equivalent environment-supplied credential) lets a scoped
  `HOME` authenticate without ever touching the real one's stored session.** I set an obviously
  fabricated placeholder value and confirmed it did not change the screen reached before the
  onboarding step this session stopped at (§1) — which tells me nothing about whether a *real* key
  would behave differently past that point, because I never went past that point. Unestablished,
  not assumed either way.
- **What the CLI's own authentication artifact looks like on disk once logged in**, and therefore
  whether a bridge could scope *that specific file* narrower than "the whole real `HOME`" (for
  example, bind-mounting or copying one credential file into an otherwise-fresh scoped `HOME`
  rather than granting the real directory wholesale). This is a plausible narrower form I did not
  get to test, because testing it means reaching an authenticated state first — the same blocker
  as the two items above.
- **What a running `claude` process actually executes once past onboarding** — whether it shells
  out to `git`, a formatter, or anything else that would need more of `PATH` than
  `/usr/bin:/bin` supplies in real operation, as opposed to at the startup stage this session
  measured. Out of scope for *this* document by the brief's own framing (PATH was already
  settled), but worth stating plainly as a B1 question this session's own method could not reach
  either.

## Conclusion for the owner

**`HOME` is the one new thing a real bridge needs, and the safe form of granting it is not the
real directory — it is a fresh, bridge-owned, throwaway one, confirmed to work, not merely
proposed.** The real `HOME` would hand a third-party process this project does not control
whatever the user's own account keeps — their CLI credentials and configuration, and everything
else that conventionally lives there — which is a different, larger decision than "let the CLI
run." This session could not establish whether the scoped form survives contact with real
authentication, and says so rather than assuming it does. **If it turns out a working, supervised
session needs the real `HOME` after all — because no narrower credential-scoping exists — that is
itself the finding D1's premise has to survive, not a detail to soften.** This document does not
yet say which of those two futures is true; it says exactly how far "trying it" got before hitting
a boundary this slice was told to stop at.

## Boundaries honoured

No prompt, conversation, or task was ever sent to `claude`; every process was launched with
closed stdin and killed by a hard deadline (4-6 seconds) before any input could reach it, verified
by inspecting the captured output, not assumed from the deadline alone. No `~/.claude/settings.json`,
`~/.claude.json`, session, transcript, or credential file was read by me directly — the one
instance where the CLI's own reading of its config became visible in my output (the disclosure
above) was the CLI's own act, surfaced as a side effect I did not anticipate, disclosed rather than
hidden, and not repeated after the method changed. The installation was not modified: nothing was
written to `~/.local/bin`, `~/.local/share/claude/`, or `PATH`; every write this session caused
landed under a scoped `/tmp` directory this session created and owns. All work happened in
`/tmp/t684-cli-env-probe/` and its own subdirectories, never in this repository or any real
project directory. Nothing under `crates/` changed, and no `.rs` file was touched.

---

## Addendum, 2026-11-15 — the reviewer, from documentation alone. §4's three questions are answered, and the escalation they would have needed is not required

The owner permitted a measurement that operates the CLI past the first-run screen, to settle §4.
**It is not needed.** All three questions are answered by
<https://code.claude.com/docs/en/authentication.md>, read rather than operated — no keystroke, no
session, no quota.

### §4.1 — is an authenticated session reachable from a scoped `HOME`?

**Yes, by two documented routes that need no browser and no interactive login.**

- **`CLAUDE_CODE_OAUTH_TOKEN`** — *"a one-year OAuth token"* from `claude setup-token`, intended
  for *"CI pipelines, scripts, or other environments where interactive browser login isn't
  available."* Set as an environment variable; nothing on disk is required.
- **`ANTHROPIC_API_KEY`** — skips the login prompt, but **not silently**: *"In interactive mode,
  you are prompted once to approve or decline the key, and your choice is remembered."* A bridge
  running an interactive session would meet that prompt once per fresh config directory.

For a bridge, `CLAUDE_CODE_OAUTH_TOKEN` is the cleaner route: no approval prompt is documented for
it. Both rank above a `/login` credential in the stated precedence order.

### §4.2 — would `ANTHROPIC_API_KEY` alone change the picture?

Answered above: it authenticates, with a one-time interactive approval. Note the trap in the same
page — *"If you have an active Claude subscription but also have `ANTHROPIC_API_KEY` set… Claude
Code uses the API key once you approve it"* — so a bridge that sets it would silently change which
account a user's runs bill to. That is a disclosure question for B2, not a blocker.

### §4.3 — what does the on-disk auth artifact look like?

`~/.claude/.credentials.json`, mode `0600` on Linux. **And the fact that matters more than the
path**:

> *"If you've set the `CLAUDE_CONFIG_DIR` environment variable, Claude Code keeps the
> `.credentials.json` file under that directory instead… and keys the macOS Keychain entry to that
> directory too, so a session with a different `CLAUDE_CONFIG_DIR` reads a different entry."*

### What this changes about the proposal

**`CLAUDE_CONFIG_DIR` is a better instrument than a scoped `HOME` alone, and the two are
complementary rather than alternatives.**

- Scoped `HOME` is still required — §1 established that, and the reviewer's own
  `env -i node -e 'require("os").homedir()'` check confirmed that leaving it unset grants the real
  home rather than withholding it.
- `CLAUDE_CONFIG_DIR` scopes *the CLI's own configuration and credential directory* explicitly, by
  a documented variable whose stated purpose is exactly this — the page's own worked example is
  keeping a work account and a personal account apart on one machine.

So the recommendation becomes: **a fresh scoped `HOME`, plus `CLAUDE_CONFIG_DIR` pointing inside
it, plus `CLAUDE_CODE_OAUTH_TOKEN` for authentication.** Each of the three is documented, none
requires the real home, and the re-onboarding cost §3 named is the one thing a seeded template
still has to address.

### One thing this addendum does not establish

Whether a *sign-in that has already happened elsewhere* can be carried into a scoped config
directory without copying credential material. The documented route is the reverse — mint a token
deliberately (`claude setup-token`) and pass it in — which is also the route that never copies
anything out of the user's own home. **Nobody should be copying `.credentials.json`**, and this
addendum is not a step toward doing so.

### Why this was read rather than run

The permission to operate the CLI was granted and spending it would have produced a worse answer:
an observation of one machine's behaviour, where the documentation states the contract. The
escalation stays unspent and available if a later question genuinely needs it.
