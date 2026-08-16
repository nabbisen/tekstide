---
title: "RFC-022 — What the dialog must not lie about: implementation handoff"
rfc: "RFC-022"
rfc_file: "../../proposed/022-adapter-spawn-and-command-approval-surface.md"
status: "Required reading before any RFC-022 code"
created: "2026-08-16"
---

# The most dangerous thing this project has rendered

Every previous surface rendered either Tekstide's own state or content the user chose to
look at. **This one renders a command an adapter chose, in trusted chrome, and asks the user
to authorise it.**

If the rendering and the execution disagree, the user authorises something they did not
read. Nothing else in this codebase has that property.

## 1. The proposed command is untrusted, attacker-influenceable text

`CommandProposal` carries `argv` and a working directory that came from the adapter. An
adapter is a cooperating process, but "cooperating" is an assumption about intent, not a
guarantee about content — and the adapter's own input may be attacker-controlled (a prompt,
a file it read, a web page it fetched).

**So the proposed command is escaped, using `text_safety::quote_untrusted` and
`DisplayText`, exactly like every other piece of untrusted text in chrome.** This is the
established rule, not a new one. It is named here because this is the position where getting
it wrong has the worst consequence.

**Corrected 2026-08-16 (response 221), because the original wording below was wrong for this
model.** This section said escaping "happens at the widget, not in the model," reasoning by
analogy with `DiffContent` and the transcript reader, where model-side escaping destroys the
answer to *what is actually in the file*. **That analogy does not transfer.** Risk
classification must run on the **raw** `argv`, so RFC-021 correctly keeps the raw form for
deciding and derives an escaped `display_command` for rendering — raw where meaning matters,
escaped where rendering happens. Do not re-derive it.

**The real rule: escape at the widget whatever the model has not already escaped.** And the
field that matters is not the one the original text was aimed at:

| Field | State | Widget's job |
| --- | --- | --- |
| `display_command` (argv) | **escaped by the model** (`display_argv`, ten-probe suite) | render it; cite RFC-021, do not re-prove |
| `cwd` | **raw `PathBuf` from the adapter** | **escape it** — this is the live attack |
| `environment_summary` | check provenance | treat as `cwd` if adapter-derived |

**`cwd` is arguably the sharper target than `argv` ever was.** A user reads the command
carefully and reads the directory to confirm context — a skim-check is exactly what a
rendering attack aims at. A path that displays as `/home/you/project` while being something
else is the whole Trojan Source case, in the field nobody escaped.

**The concrete attack:** a proposed `argv` containing a bidi override renders as something
benign while executing as something else. `rm -rf ~/project` can be made to *display* as a
harmless-looking string. The user approves what they read; the adapter runs what was sent.

### Evidence owed

- **The falsifiable claim, tested**: a proposal whose `argv` contains a bidi override
  renders it visibly as an escape marker, not as reordered text. State it as a claim that
  could be false.
- **No double-escaping** — a proposal containing the literal text `<U+202E>` stays
  distinguishable from a real override.
- **Escaping happens at the widget**, not in the model. `CommandProposal` keeps raw bytes,
  for the same reason `DiffContent` and the transcript reader do: a model that pre-escapes
  makes "what was actually proposed" unanswerable, and the audit record needs the real
  value.
- **Ablate it**: remove the escaping call, show the specific rendering difference against a
  real override.

## 2. What the dialog may claim, and what it may not

**May claim:** this command was proposed, by this run; approving it sends an approval back;
rejecting it sends a rejection.

**May not claim, or imply by omission:**

- **That rejecting prevents execution.** It does not. Nothing in Tekstide intercepts
  execution — a rejected adapter can run the command anyway, and Tekstide would not know.
  RFC-021 states this and calls the model *cooperative, not enforced*.
- **That approval makes a command safe.** The risk classification is a heuristic over argv.
- **That the command shown is all the adapter will do.** It is one proposal among however
  many the adapter chooses to make, and it may make none for its next action.

**The cooperative limit belongs on the surface**, in words a user reads, not only in
documentation. A dialog that looks like a security control while being an honour system is
worse than no dialog: it manufactures confidence that is not warranted.

Pick that wording and justify it. It is the highest-consequence sentence in this RFC, the
way the not-a-diff label was in RFC-020.

## 3. The token is not what a reader will assume

RFC-022 decides environment delivery, and the reasoning matters here because it constrains
what may be said:

**The capability token authenticates *which run is asking*, not *that the asker is
trustworthy*.** Against a hostile process running as the same user it is worthless — that
process can read `/proc/<pid>/environ`, and more to the point it can simply run the command
itself without asking anyone.

**So do not describe the token as a security boundary**, in code comments, in the surface,
or in the closeout. It is a correlation mechanism inside a cooperating system. Every
alternative delivery channel — file descriptor, socket handshake, permissioned file —
bootstraps through the environment anyway and is equally transparent to the same user, which
is why environment delivery was chosen rather than tolerated.

## 4. The reference adapter must not become evidence of something else

It exists to make the pathway demonstrable. It is written by this project, speaks a protocol
this project invented, and proves that the socket, the token, the spawn path and the dialog
work together.

**It proves nothing about real AI CLIs**, none of which speak this protocol. A closeout that
shows the reference adapter working and describes it as "command approval working" would be
true in a way that misleads.

Say what it is wherever it appears: a test-and-proof artifact.

## 5. Modal exclusivity applies, and this dialog is not user-initiated

RFC-018's rules carry over: while the dialog is open, terminal input is not produced
(`SubscriptionMode::for_modal`, plus the `is_none()` guard at the write site), and the scrim
dims chrome no terminal pane can draw into.

**One thing is genuinely new.** The paste dialog appears because the user pasted. This one
appears because an *adapter* decided to ask, at a moment the user did not choose — possibly
mid-edit, possibly while they are typing into a terminal.

That is an owner-level UX question (RFC-022 open question 3) and it is **not yours to decide
silently**. If the implementation forces a choice before the answer arrives, raise it.
