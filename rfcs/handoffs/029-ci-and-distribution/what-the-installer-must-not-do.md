---
title: "What the installer, the CI job and the landing page must not do"
source_rfc: "../../done/029-documentation-ci-and-release-automation.md"
---

# What the installer, the CI job and the landing page must not do

Three new surfaces face outward at once: a script that runs on a stranger's machine, a gate that
decides whether work is good, and a page that makes claims to people who have never run the
program. Each can lie in its own way.

## The installer

### §1 It must not do anything it did not say it would do

D12. It installs one binary to one location. **No shell rc edits, no `sudo` by surprise, no
repositories added, no telemetry, nothing removed.** If a future version needs to do more, it says
so before doing it and stops — it does not do it quietly because the user already typed the
command.

The scope must be written where the command is offered, not only inside the script. A user deciding
whether to run it is reading the install page, not the source.

### §2 It must not half-install

D11. A truncated download piped to `sh` executes a prefix. **Fail closed**: the whole body in a
function invoked on the last line, or an equivalent that cannot act on a partial file.

**Prove it by truncating it.** Serve a prefix, run it, show nothing was installed. Reading the
wrapper and agreeing it looks right is not the proof — this project has a standing rule that a
check must be shown able to fail.

### §3 It must not describe its checksum as more than it is

D10 puts verification inside the script, which is right: it happens whether or not the user would
have done it. But **the checksum is published from the same release as the binary.** It proves the
download arrived intact. It does **not** prove the release was not tampered with, because an
attacker who can write one can write the other.

So the script, the page and the release notes may say *verified against its published checksum*.
None of them may say *verified*, *secure*, *safe*, or *signed*. Signing is explicitly out of scope
(it needs an identity and a key policy), and until it exists no wording may imply it.

### §4 It must not become the only path

D13. The four-line form stays documented — for people who want it, and so that anyone can see what
the script does without reading shell. It is presented as the **equivalent**, never as the safer
one, because on the same trust root it is not.

## The CI job

### §5 It must not be red when nothing is wrong

D1, and the reason PR-029-A comes first. A gate that cries wolf teaches people to ignore red, and
everything this project does rests on red meaning something. **If the dispositions are not done,
the job is not blocking yet** — say so rather than landing it and hoping.

### §6 It must not prove less than it appears to

D2. It runs what the local gate runs — `fmt`, `clippy -D warnings` with `--all-features`, doc
tests, the full workspace with `--all-targets --no-fail-fast`, a short fixed `TMPDIR`, and **0
fixture entries left**. A green badge over a weaker subset is a claim about a gate that was not run.

### §7 It must not quietly become a substitute for judgement

D4. The release checklist's human steps stay human — the Run It gate, the book-against-the-changelog
sweep, the acceptance decision. **`0.12.0` passed every mechanical check this project had while
shipping a board full of inert buttons.** A pipeline reporting green is exactly what that failure
mode looks like from the outside.

## The landing page

### §8 It must not outrun the product

D8. It carries the platform statement and the no-screen-reader statement, and may not imply a
platform, a capability or a guarantee that does not exist. In particular it may not describe
command approval as supervision, enforcement or sandboxing — `REQ-AGENT-013` binds the landing page
exactly as it binds the book.

Marketing copy drifting from the product is the same defect class as a stale book page, and the
release checklist's *Book Against The Changelog* sweep must grow to include this page, or it will
drift the first time a feature changes.
