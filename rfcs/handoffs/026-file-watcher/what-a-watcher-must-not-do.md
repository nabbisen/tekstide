---
title: "RFC-026 — what a watcher must not do"
rfc: "RFC-026"
rfc_file: "../../accepted/026-file-watcher.md"
source_rfc_status: "Accepted 2026-09-30 — M13"
target_milestone: "M13"
created: "2026-09-30"
---

# What a watcher must not do

A watcher is a thing that reacts, on its own, to whatever a project does to the filesystem. Four ways
that goes wrong here, and the first is specific to this codebase.

## 1. It must not turn a build into a subprocess storm

**A re-scan is no longer cheap.** RFC-055 made every directory scan ask git: the gate's configuration
half (~1.3 ms) and one `check-ignore` (~1 ms). A thousand files landing in a watched directory must
cost **one scan**, not a thousand — which is about two and a half seconds of `git` if you get it
wrong, on a machine that is already busy compiling.

So the batching is not a performance nicety bolted on afterwards; it is the first slice, and it is
proved against a **simulated** event stream before a real watcher exists (D11). The evidence is two
counted numbers — scans, and git subprocesses — not the word "batched".

## 2. It must not assume the kernel will keep saying yes

Watches are a **per-user kernel resource** and the limit is not ours to choose. This machine allows
524,288 with no `sysctl` override, which means another machine's may be far lower — and the
interesting case is the one we cannot reach by waiting.

**Force it.** A test that makes `inotify_add_watch` fail, and asserts three things: no crash, the
explorer falls back to exactly today's behaviour (stale until reopened), and **the sidebar says
watching stopped and why**, in the vocabulary RFC-055 already uses when git cannot answer. Silence is
the failure mode to guard against, because a watcher that has quietly stopped looks exactly like a
project where nothing is changing.

## 3. It must not become a second way into the filesystem

The explorer's access policy decides which paths are readable. **The watcher watches only those.**
`REQ-SEC-043` names unbounded recursive symlink loops; a symlink that leaves the project root is not
watched, and a loop does not make the watcher recurse. The fixture is hostile and the test reads what
happened, not what the crate's documentation promises.

And watches must be **dropped** when what they watch goes away — a collapsed folder, a deleted
directory, a closed project. A scope that only grows reaches §2 even on a generous machine.

## 4. It must not make an external change arrive *differently*

The states already exist and are proved: `ExternalChanged`, `Conflict`, and a Reload path that
constructs a **fresh** `TextDocument` — which is why undo cannot cross a reload (RFC-057 PR-057-D).

A watcher's whole job is to make that state arrive **sooner**, without the user reopening the folder.
It must not invent a second route to it, and in particular:

| | |
| --- | --- |
| A file changes on disk while the user has unsaved edits | the edits survive, and the existing conflict path decides what happens — **never a silent reload** |
| A reload does happen | the undo history goes with the document that went, and the product does not pretend otherwise |
| A file the user has open is deleted | a state the product can say, not a panic and not a stale buffer claiming to be clean |

`NFR-REL-005` is the requirement undo was added under. A watcher that discards a user's typing
because a build touched the file is that same requirement failing, from the other direction.

## 5. It must not react to its own reads

**Found in D8, not merely feared: notify subscribes to `OPEN` on every watched directory.** The
document refresh reads its own file to decide whether it changed, and that read is itself an event in
a directory the project watches. Without a filter, each refresh was answered by a notice asking for
another refresh — the first D8 measurement ran for two minutes after its burst had ended, with 239
scans where three were expected. The fix drops access events at the consumer (`WatchEvents`), not by
narrowing the platform subscription, so it holds whatever the platform decides to report.

**The same shape, smaller and not a defect: every save now costs a scan and a whole-file re-read.**
`write_text_via_temp_rename` writes inside the document's own parent, which is watched whenever that
document is open. That write is a real, intended change — the filter does not drop it — and the
refresh it triggers finds the text matches disk and the document stays `Clean`. About 4 ms at the
3.3 MB fixture, every save, worth naming before someone measures saving and is surprised. (Review 464.)

## And the one that is not about watching

**`open_buffer_count()` and `dirty_file_count()` are each `u32::from(<one Option>)`**, with one reader
each. When the open set becomes plural those two must count **all** of it — and a count that is wrong
still compiles. The test that matters fails if either counts one when two are open.
