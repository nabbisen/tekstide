---
title: "RFC-069 PR-069-A — the upstream interface, measured"
rfc: "RFC-069"
rfc_file: "../../accepted/069-an-adapter-for-a-real-ai-cli.md"
source_rfc_status: "Accepted by the human owner 2026-11-15 — RFC-069 is in rfcs/accepted/"
target_milestone: "pre-1.0"
created: "2026-11-15"
---

# PR-069-A: the upstream interface, measured

No product code. No `.rs` file touched, no profile changed, nothing in `crates/`. This document is
the whole deliverable, per D2 and the pack's own scope.

**CLI measured**: `claude` (Claude Code), the binary the built-in `claude-code` profile already
resolves at `$HOME/.local/bin/claude` — the same install `agent/profile.rs`'s own comment calls
"the real install location confirmed on the reference dev machine," not a second copy fetched for
this slice.

**Every fact below traces to a command I ran on this machine, or a public documentation page I
read — named at each claim, not asserted from memory.** I did not read `~/.claude/settings.json`,
`~/.claude.json`, any session, transcript, or credential file for this CLI, and I did not modify
this installation. Where the public documentation did not answer a question I asked it, that is
recorded as **unestablished**, not filled in from what would be plausible.

## 1. The mechanism

**Claude Code's `PreToolUse` hook.** A `PreToolUse` hook "run[s] before the permission prompt, for
every tool except [`EndConversation`]" — <https://code.claude.com/docs/en/permissions.md>, the
"Extend permissions with hooks" section. It receives the proposed tool call on stdin as JSON and
can return a decision, before Claude Code's own prompt or auto-approval logic ever runs.

**Confirmed independently from the CLI's own `--help`** (`claude --help`, run locally
2026-11-15), without reading any external page: the `--bare` flag's own description names hooks as
a real, existing thing it disables (*"skip hooks (those defined in settings and by installed
plugins; features built into Claude Code are unaffected)"*), and so does `--safe-mode`'s own
description (*"all customizations (CLAUDE.md, skills, installed plugins, hooks, ...) disabled"*).
The mechanism is not something only the external docs claim exists.

**Activation, and how it is reached without writing to the user's installation.** A hook is
registered under a `hooks.PreToolUse` key, an array of entries shaped (literal example, quoted
verbatim from <https://code.claude.com/docs/en/hooks-guide.md>, "Set up your first hook" /
"Block dangerous commands" sections):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/block-rm-rf.sh" }
        ]
      }
    ]
  }
}
```

This JSON is ordinarily placed in `~/.claude/settings.json` or a project's `.claude/settings.json`
— both of which this RFC's own boundary and D1's *Out of scope* ("not modifying the user's CLI
installation, ever — no config rewriting") forbid writing to. The CLI's own `--help` names a third
place that writes nothing: `--settings <file-or-json>` — *"Path to a settings JSON file **or a
JSON string** to load additional settings from."* A bridge launches `claude --settings
'{"hooks":{"PreToolUse":[{"matcher":".*","hooks":[{"type":"command","command":"<path to the
bridge's own helper>", ...}]}]}}'` as a subprocess argument. Nothing is written to disk, and
nothing in the user's own `~/.claude/` or project `.claude/` is touched — the mechanism this
project would bind to does not require modifying the installation to register. I did not run this
myself (that would be bridge code, out of scope for this slice); I traced the flag's own
documented behavior and the hook registration shape as two separately-confirmed facts and combined
them. **Whether `--settings`'s own hook entries compose correctly alongside a project's own
`.claude/settings.json` hooks (both firing, neither silently replacing the other) is stated in
general terms by the docs — "Hook entries merge across settings levels rather than replacing each
other" (<https://code.claude.com/docs/en/hooks.md>) — but I did not run both at once to observe
it, so I record the general claim and mark the specific case unobserved.**

**Sources**: <https://code.claude.com/docs/en/hooks.md>; <https://code.claude.com/docs/en/hooks-guide.md>;
<https://code.claude.com/docs/en/permissions.md>; `claude --help` (run 2026-11-15).

## 2. The version it was observed on

```
$ claude --version
2.1.325 (Claude Code)
```

Run locally, 2026-11-15. Exact stdout, one line, no trailing content beyond a newline (checked with
`claude --version | cat -A`). This is the only version installed on this machine; nothing here was
cross-checked against a second installed version.

## 3. How that version is detected at runtime, and how narrow it can safely be

**Detection**: spawn `claude --version` (or `-v`, same output per `--help`) as a subprocess before
the real session launches, and parse stdout against the exact string observed in item 2 — the only
shape I have seen it take.

**How narrow is safe, measured rather than guessed.** The public hooks reference states several
sub-features this bridge would depend on as gated to a specific patch version, quoted verbatim:

- *"requires Claude Code v2.1.295 or later"* — for `onFailure: "block"` on a `command`/`http` hook,
  which is the **only** documented way to make a hook's own failure fail closed (see item 5).
- *"Requires Claude Code v2.1.292 or later"* — for a tool requiring user interaction still
  prompting even when a plugin "mod" approves the call.
- *"Requires Claude Code v2.1.314 or later"* — for `permissions.blockWritesOutsideWorkingDirectories`,
  and separately for a Bash-matcher `PreToolUse` hook to see a `Monitor`-started command at all.
- *"(v2.1.299+)"* — for `CLAUDE_CODE_DISABLE_FILE_MENTIONS`, which is the only way to make an
  `@`-mentioned file read go through a tool call (and therefore through a hook) at all — see item 4.

**Conclusion, stated as a testable claim, not an impression**: pinning at the `major.minor` level
(`2.1`) is not safe, because the one feature D3's own fail-closed requirement depends on
(`onFailure: "block"`) did not exist for the whole `2.1` line — it needed `2.1.295`. The measured
version, `2.1.325`, is 30 patch releases past that floor, which is margin, not proof that nothing
between `2.1.295` and `2.1.325` changed the feature's own behavior — I observed exactly one point
on that range. D5's own text says this will refuse on a harmless patch bump and that this is the
annoying failure its override exists for; this measurement does not narrow that down further than
"pin the exact string observed, refuse everything else by default, let the user override it."

I found no documented capability-probe endpoint (a `claude --features` or similar machine-readable
capability list) that would let the detection be narrower than a version-string match. I read the
full `--help` output and fetched the hooks and permissions reference pages, but those two pages
are large (the hooks reference alone is close to 300,000 characters) and my own reading of them
was in chunks via a fetch-and-summarize tool, not a byte-for-byte pass. **This is recorded as
"found none in what I read," not as a confirmed absence across the entire documentation set.** A
`claude --help`-adjacent command I did not try is `claude doctor` run for real rather than only
its `--help` text; I did not run it because it reads settings files in the current directory and
I was not confident it would stay within the self-description boundary. **Also left unestablished,
not assumed negative.**

## 4. What the mechanism does not cover

Specific findings, each sourced, not an impression that "most things are covered":

- **`EndConversation` calls never reach `PreToolUse` at all.** *"Matches on any tool name except
  `EndConversation`"* — <https://code.claude.com/docs/en/permissions.md>. A session-ending action
  is invisible to this mechanism by name, not by omission.
- **A file referenced with `@` in the user's own prompt is never a tool call, so no hook fires for
  it — "including hooks matching `Read`"** (same page), unless the operator sets
  `CLAUDE_CODE_DISABLE_FILE_MENTIONS=1` (itself gated to v2.1.299+), which routes those reads
  through the normal tool path at the cost of disabling that convenience feature entirely.
- **A call rejected by validation before any hook runs is invisible to this mechanism**: *"tool
  calls rejected before execution: an unknown tool name, input that fails schema or tool-specific
  validation, or a permission denial... fire neither `PreToolUse` nor `PostToolUseFailure`."* Less
  concerning in practice (the call did not execute), but it means a bridge cannot audit *attempted*
  malformed calls through this path.
- **`--bare` disables hook loading entirely, confirmed from two independent sources**: the CLI's
  own `--help` text for `--bare` ("skip hooks... from... settings and... installed plugins") and
  the permissions reference's own headless-automation guidance, which names `--bare` as a way to
  make *"Claude Code read[] no hooks, skills, custom commands, subagents, plugins, or `.mcp.json`
  servers from the project."* If the real `claude` process is ever started with `--bare` by any
  path other than the bridge's own controlled invocation, interception is silently absent — no
  error, no refusal, just no hook. D3 requires the bridge to control exactly how the child is
  launched and never pass or inherit `--bare`.
- **The mechanism's own default failure mode is fail-OPEN, not fail-closed — the single most
  load-bearing finding for D3.** *"a timed-out `command`, `http`, or `mcp_tool` hook doesn't block
  the tool call... The call continues through the normal permission flow, so don't count on a
  stalled hook to act as a gate."* Crash, non-2xx HTTP, invalid JSON output, or exceeding the
  600-second default timeout all behave the same way by default: **the tool call proceeds as if no
  hook had ever been registered.** The only documented escape from this is the `onFailure: "block"`
  field named in item 3, itself version-gated. **D3 ("the AgentRun does not launch" on any
  mismatch) is achievable here only by setting `onFailure: "block"` explicitly on every hook entry
  this bridge registers — it is not the mechanism's own resting behavior, and a bridge that forgot
  this field would silently satisfy D3's own words ("fails closed") exactly backwards.**
- **A second, independent way for a bridge to believe it blocked a call and not block it: the
  conventional Unix failure exit code is the wrong one.** `onFailure: "block"` (above) governs
  what happens when the hook *fails to run its own logic* — crash, timeout, bad output. It is a
  separate question from what happens when the hook *runs and decides to deny*, which is governed
  by exit code alone for a `command` hook that signals its decision that way rather than through
  `hookSpecificOutput` JSON: *"Exit code 1 without valid JSON output is a non-blocking error unless
  the hook is one whose failure blocks the action, even though 1 is the conventional Unix failure
  code. If your hook is meant to enforce a policy, use `exit 2`."* A bridge that gets `onFailure:
  "block"` exactly right, then signals a denial by exiting `1` — the instinct most engineers would
  reach for first — lets every denied call through anyway. **Both must be got right at once**: the
  hook's own failure path (`onFailure: "block"`) and the hook's own decision path (`exit 2`, or
  `hookSpecificOutput.permissionDecision: "deny"`, never a bare `exit 1`).
- **A hook's own `"allow"` is not absolute.** It *"skips the permission prompt, except for actions
  no mode auto-approves and reads from network paths,"* and does not suppress the prompt for
  `always_ask` tools, MCP tools marked `requiresUserInteraction`, or, in plan mode, any
  non-read-only action. This cuts the other way from the findings above: Claude Code's own
  permission system remains a second, independent gate in some categories even when this bridge's
  hook is present and working — relevant to D4, since it means "Managed" cannot honestly be
  described as making this project's own approval the *only* thing standing between a proposal and
  execution in every case.
- **On an ordinary personal install — no managed settings, no `managed-mcp.json`, no Team/
  Enterprise `/login` session credential, which is the shape this product ships against — a
  plugin "mod" handling the `tool.check` event can override this bridge's own deny.** Quoted
  precisely: those three protections only apply *"with managed settings... or a `managed-mcp.json`
  file... or a Team or Enterprise `/login` as the session's credential"* — under them, *"a call
  that a `PreToolUse` hook blocked stays blocked."* Without them (an ordinary API key or a
  long-lived `CLAUDE_CODE_OAUTH_TOKEN`, the authentication shape an individual user has), *"the
  mod can approve a call that a deny rule refuses or a `PreToolUse` hook blocked."* So if the
  user ever installs a mod that handles `tool.check`, this bridge's own deny is not guaranteed
  final on a personal account — it is final only under enterprise-managed configurations this
  product does not assume. A real, confirmed gap, not an unresolved question.
- **Whether `--settings`'s own `disableAllHooks` (or a hook registered through it) can be
  silently defeated by a project's own `.claude/settings.json` is not answered in the direction
  that matters here.** The docs state the reverse case (a user wanting hooks off via `--settings`
  can be overridden by a project turning them back on) but I found no statement about a project
  turning hooks off after `--settings` turned them on. **Unestablished — not assumed safe.**
- **`onFailure` is not universal even where it does apply.** It *"has no effect on"* `Stop`,
  `SubagentStop`, `TaskCompleted`, or `TeammateIdle` hooks, or on a background `command` hook
  running with `async`/`asyncRewake` set. None of those is `PreToolUse`, so it does not weaken
  this slice's own finding — but "the one setting D3's fail-closed behaviour rests on has
  documented exceptions elsewhere in the same mechanism" belongs on the record.
- **A Windows exception exists and is irrelevant today, but not permanently.** Without Git Bash
  installed, a shell-form `PreToolUse` command hook that fails with `onFailure` unset can still
  block the call, unlike the Linux/macOS default. Tekstide ships neither a Windows build nor a
  Windows profile today (RFC-071 is reserved, not written); recorded now so whoever writes that
  RFC does not have to re-derive it.

**I looked for a case where the honest answer was "found none" and did not get one.** Every item
above is a real, specific gap or open question, sourced to a quote; this is not a thin pass over a
mechanism that turned out fully covered.

## 5. What happens when it is absent or fails

Already established with sources in item 4's fail-open finding: by default, an unreachable,
crashing, or timed-out hook lets the tool call proceed through Claude Code's normal permission
flow, with a `<hook name> hook error` notice (naming the first stderr line) shown for a hook that
started and exited with a code other than 0 or 2, and no distinct on-screen signal documented for
a pure timeout beyond the same general notice.

**For D3's fail-closed requirement, this is not optional configuration — it is the one setting
the whole decision rests on**: every `PreToolUse` hook entry this bridge registers must carry
`"onFailure": "block"` (available on the measured `2.1.325`, per item 3's version floor). Without
it, "the mechanism cannot establish the interception it expects" (D3's own words) is exactly the
state that silently falls through to an unsupervised run — the opposite of fail-closed.

## Conclusion for D1/D2

**The mechanism exists, is real (confirmed from the CLI's own `--help`, not only external docs),
and is reachable without modifying the user's installation** (`--settings` as an inline JSON
string). It is narrow in the way D5 anticipated (refuses on a harmless patch bump) and has several
specific, named gaps (item 4) a bridge built on it must disclose rather than paper over, per
`what-the-dialog-must-not-lie-about.md`'s own standard. **D3's fail-closed behavior is achievable,
but only by getting two independent things right on every registered hook entry — `onFailure:
"block"` for the hook's own failure path, and `exit 2`/`permissionDecision: "deny"`, never a bare
`exit 1`, for the hook's own decision path** — neither is the mechanism's own default, and a
bridge that nailed one while reaching for the conventional-but-wrong exit code on the other would
still be silently open. This is a finding the owner should weigh directly against D1's premise,
not a settled "yes, build it": a bridge is possible, but the interception it would offer is
**opt-in to fail-closed, not fail-closed by nature**; **overridable by a plugin mod on exactly
the account shape this product's own users have** (no managed settings); and **several named
categories of action never reach it at all**. What D4 may honestly claim about "Managed" is
narrower than the word alone would suggest to a reader who has not seen this document.

## Boundaries honoured

Commands run: `claude --help`, `claude --version`, `claude doctor --help`, `claude mcp --help` —
all self-description, none reading or requiring any personal settings, session, transcript, or
credential file. Public documentation read: the three URLs cited above, via `WebFetch`/`WebSearch`.
Nothing in `~/.claude/settings.json`, `~/.claude.json`, or any session/transcript/credential path
for this CLI was read. Nothing about this installation was modified. No `.rs` file, no profile, and
nothing under `crates/` changed in this slice.
