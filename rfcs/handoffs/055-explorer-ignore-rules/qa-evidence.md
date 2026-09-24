# RFC-055 — QA evidence

Written as each slice lands, per the checklist. Numbers are from this machine, 2026-09-25, against this
repository (`target/` and all) unless a row says otherwise.

## PR-055-A — the query, and the file that tries to silence it

`crates/tekstide-core/src/runtime/git.rs`: `ignored_entries(directory, names) -> IgnoreAnswer`, its tests in
`runtime/git/tests.rs`. **No surface change and no explorer change**; nothing calls it yet.

### The three answers

`IgnoreAnswer::Ignored(set)` (never empty), `IgnoreAnswer::NoneIgnored`, `IgnoreAnswer::Unknown(IgnoreUnknown)`.
`Unknown` carries a closed reason — `NotARepository`, `GateRefused`, `QueryFailed`, `UnusableName` — and nothing the
repository wrote, so a diagnostic built from it cannot echo a filename. Exit `0` with a reply → `Ignored`; exit `1`
with an **empty** reply → `NoneIgnored`; everything else, **including a reply that contradicts its exit code, one that
names a path nobody asked about, and one git never terminated** → `Unknown(QueryFailed)`.

### D3 — the file that silences a directory

`a_file_named_like_pathspec_magic_does_not_silence_its_siblings`: `:(glob)evil.log`, `a.log`, `b.txt`, `c.txt` in one
batch; the answer is `Ignored({:(glob)evil.log, a.log})` — every name answered, the hostile one included.

**The abort is real, so the test is not passing on a fixture that was never hostile**:
`a_raw_glob_named_path_aborts_the_whole_batch` feeds git the same names *without* the prefix and asserts exit
**128**, an empty stdout, and stderr containing `pathspec magic not supported` (the full line git prints is
`fatal: :(glob)evil.log: pathspec magic not supported by this command: 'glob'`).

**Ablation** (`ablate.sh`, committed tree): `QUERY_PREFIX` set to `b""` fails
`a_file_named_like_pathspec_magic_does_not_silence_its_siblings` (the answer becomes `Unknown(QueryFailed)`, because
git exits 128 for the whole batch), `every_failure_to_answer_…` (the well-behaved fake echoes `./a.log`, which no
longer matches) and `check_ignore_has_one_call_site_and_never_uses_no_index` (the prefix is no longer written once).

**Structural, not a convention.** `IgnoreQueryInput` is the only way to build a query; it takes *names*, not paths, and
has no constructor from a path. A name that is empty, `.`, `..`, contains `/` or NUL, or is over 255 bytes is
`UnusableName` and refuses the whole call before any process. The prefix is a single constant applied in one place
(`extend_from_slice(QUERY_PREFIX)`) and stripped in one (`strip_prefix(QUERY_PREFIX)`); `check_ignore_has_one_call_site_and_never_uses_no_index`
counts those, counts `"check-ignore"` (1) and the stdin runner's call sites, and fails on `no-index` or
`GIT_LITERAL_PATHSPECS` anywhere in production source.

### D4 — unknown is not "not ignored"

`nothing_ignored_is_an_answer_and_not_an_unknown` (exit 1, including a name that does not exist, which also exits 1),
and `every_failure_to_answer_is_unknown_and_never_none_ignored` — eight ways for a `git` to fail or lie (exit 128, exit
2, exit 1 *with* a reply, exit 0 with none, an unterminated record, an unasked path, a path without the prefix, killed
by a signal), each through a stand-in `git` that leaves a marker proving it was actually asked, and a ninth well-behaved
one that is an answer. **Ablations:** an unexpected exit read as `NoneIgnored` fails that test; a reply naming an
unasked path accepted fails it too.

### D5 — a tracked file is tracked

`a_tracked_file_matching_a_pattern_is_not_ignored`: `tracked.log` (force-added) and `untracked.log` under `*.log`:
only the untracked one is `Ignored`. **Ablation:** `--no-index` added to the argv fails it.

### The project root inside its repository

`a_project_root_inside_its_repository_gets_the_repository_root_rules`: `repo/sub/deep` with a repository-root
`.gitignore` of `*.log` and `build/`; asked from `deep`, `a.log` and the *directory* `build` are ignored, `b.txt` is not.
**Ablation:** searching only the directory itself for a `.git` fails it.

### R6 — bytes

`a_non_utf8_filename_survives_the_round_trip`: `caf\xe9.log` comes back with the same bytes. The reply is split and
compared as bytes and turned into an `OsString` only after the comparison.

### R3 — no repository, no work tree, no git

`not_a_repository_is_unknown_without_running_anything` (a stand-in `git` that marks *any* invocation sees none),
`a_repository_the_gate_does_not_accept_is_never_asked_anything` (also git not found → `GateRefused`, no panic), and
`a_gate_refusal_never_reaches_check_ignore` (the configuration was read; `check-ignore` was never made).

### D9 — where the gate is paid, and that it is not cached

Paid **inside `ignored_entries_with_environment`, once per call**, as `vet_configuration`.
`the_gate_is_asked_again_on_every_call` accepts a repository, adds `core.fsmonitor` to it, and asks again: the second
call is refused and the program never runs. **Ablations:** the gate not consulted fails it and
`a_repository_the_gate_does_not_accept_is_never_asked_anything`; the verdict cached in a `OnceLock` fails both.

### Findings, measured — three of them change what the RFC assumed

1. **`git check-ignore` runs a repository's `core.fsmonitor` program.** A repository with
   `core.fsmonitor = <script>`: an unprotected `git check-ignore ./a.log` touched the script's marker (git printed
   *"Empty last update token"*). It reads the index, and reading the index refreshes through fsmonitor. So the gate is
   **required for this query, not merely careful** — measurement 9 ("available exactly when `git status` is") is right for
   the reason it did not give. The hostile control is committed (`a_repository_the_gate_does_not_accept_is_never_asked_anything`
   fails if an unprotected check-ignore stops running it, so the test cannot go quietly meaningless).
2. **The whole gate is not "a few milliseconds"; on this repository it is ~87 ms warm, and 99 % of it is the gitlink check and the
   attributes walk.** One run, five repetitions, 1 ms polling: the whole gate 86.0-88.1 ms, the configuration half 1.27-1.39 ms, so
   **~86 ms** is `worktree_names_a_content_driver` (the gitlink check and a recursive walk of every `.gitattributes`, about
   239,000 entries — **96 % of them under `target/`**, per review 431's count of 239,259 with 229,384 in `target/`). *(An earlier
   draft of this file and of the source comment quoted "115 ms" and "117 ms" from separate runs at 10 ms polling and called one a
   part of the other; corrected at review 431, R2.)* RFC-055's acceptance measured `--version`, `config`, `rev-parse` and
   `check-ignore` at ~1 ms each and added them up; it did not run `evaluate`. That walk is about whether a **content comparison** can
   be trusted — the module's own doc calls it "not a safety problem" — and `check-ignore` reads no content. So the query asks **only
   the configuration half** (`vet_configuration`, split out of `evaluate` with `evaluate`'s behaviour unchanged: all 43 existing gate
   tests pass). *A call that paid the whole gate would cost ~87 ms per expansion.*
3. **Most of what remained was the poll interval.** `run_bounded` slept 10 ms between `try_wait`s, so every git call
   cost up to 10 ms more than the process. Measured, 21 entries of this repository's root, the whole call (gate + query):
   **20 ms at 10 ms polling, 2.3 ms at 1 ms** (configuration half 1.1 ms, query 1.1 ms); a 37-name nested directory 2.3 ms. I changed it
   to 1 ms, which also speeds the status-bar refresh's three subprocesses (review 431 checked that nothing else polls through it).
   **The number the 100 000-entry fixture is to be held against in PR-055-B is this whole call, ~2.3 ms, not the query alone.**

### Judgment calls, disclosed

- **Only the configuration half of the gate.** Not what the pack literally says ("exactly as `compute_summary_with_environment`
  does it"), for the reason in finding 2. It is *not* weaker for the danger the gate exists for — a program the configuration
  names — because that is the half it keeps. What it does not repeat is the gitlink check (`R1`, a submodule's own config
  consulted by `git status`): **measured, `check-ignore` does not run a poisoned submodule's filter or fsmonitor**, and
  `a_poisoned_submodule_is_not_run_by_the_query` pins that with the review-406 fixture, so a git that changes it fails a test
  rather than a user. If you would rather have the gitlink check anyway, it is one call; it costs a `git ls-files -s` per
  expansion and refuses every repository with a submodule.
- **`AcceptedBranchOnly` is not enough.** A repository with any config key outside the allowlist (`include.path`, or one nobody has
  reviewed yet -- *not* an editor's `branch.*.vscode-merge-base`, which review 407 put on the allowlist; **this repository's own
  configuration is entirely allowlisted, so Tekstide's own repository is asked**) gets `Unknown(GateRefused)` and, in PR-055-B, the floor list. That is the
  cost of finding 1: I could not allow those keys and still say the program named by `core.fsmonitor` is never run. So the
  acceptance criterion "a repository whose `.gitignore` does not name `target/` shows `target/` as expandable" holds for
  repositories whose configuration is allowlist-clean and falls back to the floor for the others. **B's capture should say which.**
- **A repository at or above `$HOME` is not asked** (`RepositoryDeclined`, its own variant since review 431 R3 -- `NotARepository`
  would be untrue of a repository that is there and was declined). A dotfiles repository at `~` with `.gitignore` = `*` would
  answer "ignored" for every entry of every project beneath it, and `show_ignored = false` would then hide the user's
  whole tree — the fail-closed outcome the risk document ranks worse. The pack does not mention it; a nested project root
  (which it requires) is what exposes it, because the search now walks *above* the project root. Ablated:
  `a_repository_at_or_above_home_is_not_asked` fails without it. Say if you would rather not have the special case.
- **The repository is the nearest `.git` at or above the canonical directory**, which is what git's own discovery finds. This is
  wider than `compute_summary`, which reads only a `.git` at the project root — so a project *inside* a repository gets ignore
  knowledge while its status bar still says Git is not available. Both are true statements; they are not the same question.
- **`MAX_IGNORE_QUERY_ENTRIES = 256`**, equal to the explorer's per-directory cap and **held equal by a test** (review 431 R5; it was
  1,024 under a comment that said 256), stated again where the subprocess is so no caller can turn a scan into an unbounded write. With the 255-byte name cap the reply cannot reach the 1 MiB pipe bound (a `const` assertion),
  so `read_bounded` truncating it — which would turn "ignored" into "not ignored" — is unreachable.

### Review 431's required items

- **R1** — `a_scan_inside_a_poisoned_submodule_is_refused_by_the_gate`: the explorer expanding a submodule asks about the submodule's
  own entries; `enclosing_repository_root` resolves its `.git` pointer file, the submodule is the repository, `vet_configuration`
  vets *its* configuration (where `filter.evil.clean` is not allowlisted), the answer is `Unknown(GateRefused)`, and the filter's
  marker never runs. The dropped gitlink check no longer rests on a reading.
- **R2** — one measurement, one pair of numbers, in `vet_configuration`'s comment and above.
- **R3** — `IgnoreUnknown::RepositoryDeclined`, with the dotfiles reasoning on the variant.
- **R5** — see above.
- **R4 (disclosure, travels to B and C): `core.excludesFile` is not honoured.** The query runs with `GIT_CONFIG_GLOBAL=/dev/null` (R6 of the
  gate, and right), so a user's own `core.excludesFile` is not read: `.gitignore`, `.git/info/exclude` and
  `$XDG_CONFIG_HOME/git/ignore` are honoured, but where a user has set `core.excludesFile` git itself would use *that* file *instead of*
  the default, so Tekstide and their `git status` disagree in both directions. **`REQ-FILE-005` is not to be marked complete without
  naming this** — half of "user ignore configuration" is out of reach — and B's or C's user-facing disclosure says so in the user's
  words. Whether to honour it (reading the user's *own* config, not the repository's, and passing `-c core.excludesFile=`) is the
  architect's decision at C; nothing here does it.

## PR-055-B — the scan carries it, and the floor keeps its job

`project/root/explorer.rs` (the model and the answer applied to a scan), `project/explorer_tree.rs` (`ExplorerScanRequest::run`
asks), `runtime/git.rs` (the report says where the repository is), `surface/explorer.rs` + `locales/en.ftl` (the word and the
rule line), `project/ignored_directories.rs` (the doc comment). **No change to `shell.rs`**: the worker that already calls
`request.run()` now asks git inside it, so nothing new runs on the render thread — and the synchronous
`scan_explorer_directory` path stays the pure scanner and never blocks on a subprocess.

### The model

- `FileGitStatus::Ignored` (D5). **One `match` on `Option<FileGitStatus>` exists in production** (`surface/explorer.rs`
  `git_status_symbol`); it has seven arms and **no catch-all**, and adding the variant was a compile error (`E0004`, pattern
  `Some(FileGitStatus::Ignored)` not covered) until it was handled. The only other production mentions are constructors in
  `runtime/git.rs` (`compute_summary` never produces `Ignored`; `git status` is not run with `--ignored`).
- `ExplorerNode.ignore: ExplorerIgnoreState` — `Unknown | NotIgnored | Ignored`. `NotIgnored` is a claim git made; `Unknown` is
  the absence of one and is never drawn as either.
- `ExplorerDirectoryScan.ignore_rule: ExplorerIgnoreRule` — **a value the scan carries**: `Git { repository:
  AtProjectRoot | AboveProjectRoot | BelowProjectRoot }` or `Floor(NotAsked | NotARepository | RepositoryDeclined | GateRefused |
  QueryFailed)`. The pure scanner returns `Floor(NotAsked)`; `ask_git` sets it. Each `IgnoreUnknown` maps to its own reason
  (`UnusableName` — unreachable for a real directory entry — to `QueryFailed`).
- `ignored_entries` now returns an `IgnoreReport { answer, repository_root }` (A's contract, extended additively: the answer is
  unchanged) so the scan can say whether the repository is the project's own, above it, or inside it.

### D6 — git decides what is collapsed; the list is the floor

Under a `Git` rule a directory is collapsed iff git says it is ignored (**or it is `.git`**); under a `Floor` rule the fixed list
decides, exactly as before. `git_decides_whether_target_is_collapsed` runs both repositories through the real git:
`.gitignore` = `*.log` gives an **ordinary, expandable `target/`** (`NotIgnored`), and `target/` + `*.log` gives it
**collapsed and `Ignored`**. Captures `01-` and `02-`. `outside_a_repository_the_floor_list_decides_and_the_scan_says_so`
and capture `03-` show the floor: `target` collapsed, `Floor(NotARepository)`, every node `Unknown`.
**Ablation:** ignoring git's collapse decision fails `git_decides_whether_target_is_collapsed` and
`unknown_and_not_ignored_are_different_on_the_node_and_in_the_rule`.

**`.git` stays collapsed under either rule** — a judgment call. git does not call `.git` ignored (it is not something a
`.gitignore` names), so under a strict reading `.git` becomes an ordinary expandable directory and a click opens
the repository's internals. `dot_git_stays_collapsed_under_git_s_rule`; ablated by removing the exception.

### D2's falsification, and the omitted tail

`what_git_is_asked_about_tracks_the_rows_drawn_not_the_ignored_file_count`: **20,000** `*.log` files, one-line `.gitignore`. The
scan returns 256 nodes, git was asked about **256 names in one query**, all 256 are `Ignored`, `omitted_entries` is 19,744, and
`nodes.len() + omitted_entries == 20_000` — the omitted rows were never in the batch and the count makes no claim about
them. **D2 holds**: it would have failed had the batch tracked the ignored-file count.

### The query is not a second way into the filesystem

`a_blocked_entry_is_never_asked_about_and_stays_unknown` (an escaping symlink named `escape.log`, which the pattern would
match, is `Blocked`, absent from the batch and `Unknown`) and `only_admitted_entries_are_put_in_the_batch` (an unreadable
placeholder row). **Ablation:** asking about every entry fails both. Names are taken from `relative_path`'s raw filename, not
the lossy display `name` (`a_non_utf8_name_is_asked_as_its_bytes_and_the_answer_lands_on_its_row`; ablated by asking with the
display name).

### The worker entry point

`the_worker_entry_point_asks_git_and_says_what_it_learned`: through `ExplorerTree::request_scan` → `scan_requests` →
`ExplorerScanRequest::run`, in a directory in no repository, the scan is `Floor(NotARepository)`, **not** `Floor(NotAsked)`.
**This test exists because an ablation found nothing failing**: with `run` not asking git at all, every other test still passed,
because each supplies its own oracle. A producer with no consumer test; it now has one.

### Budget — RFC-052's 100,000-entry fixture, the whole call

`asking_git_adds_a_few_milliseconds_to_a_hundred_thousand_entry_scan` (the fixture project made a repository; best of 5, this
machine): **scan alone 21.75 ms; asking git (the gate + one `check-ignore` over 256 names) 2.86 ms; whole call 24.61 ms.** The
scan is the 100,000-entry directory read and drain, unchanged from RFC-052; git adds ~12 % to it, about a sixth of one frame, and
none of it on the render thread. The assertion is deliberately loose (`< 250 ms`, a wall clock under load is not a property); the
number is here. The gate is the configuration half only (A's evidence, finding 2); the full gate would have added ~87 ms.

### Review 431 ruling 4 — the two surfaces may not look like one answer

The status bar's Git state reads only a `.git` at the project root; a project inside a repository now gets ignore words from the
repository above it. **When the rule came from a repository that is not the project's own, the sidebar says so**, from what the
loaded scans carried (`ignore_rule_line`, read from `ExplorerTree::loaded_scans`): *"Ignore rules: parent Git repo"* /
*"Ignore rules: nested Git repo"*. Capture `04-` shows a project two levels inside its repository: `[ignored] a.log` from the parent's
`.gitignore`, the sentence above the tree, and the status bar saying **Git: not available** — two true statements, and the sidebar
says why they differ. The pack put the words in C; the ruling put this part in B, and I followed the ruling. C still owns the
floor's sentence and the age of the answer.

**Found by the capture, not by a test:** my first wording (*"Ignore rules come from the Git repository above this project."*) was cut
off at "the Git r" — tree rows are unwrapped and the sidebar is ~33 monospace columns. It is now 29 characters, and
`the_ignore_rule_sentence_fits_the_sidebar` holds it to 32. `RESERVED_LINES` went from 2 to 3 for the line (reserved whether or
not shown, so a scan finishing does not move the window).

### Found by the capture: an ignored directory's row now clips its name (disclosure)

Capture `02-`: `> [+] ▣ (collapsed) [ignored] t` — the row for `target` carries **two** status words and the sidebar cuts the name to
`t`. The existing limitation (a row wider than the sidebar is clipped; the detail line under the tree shows the highlighted row
whole) was already documented for `0.24.0`, but every ignored *directory* under git's rule now has two words, so it will be met
constantly, not rarely. I did not fix it: options are dropping `(collapsed)` when `[ignored]` says the same thing, ordering
the name first, or a wider sidebar — a layout decision. **It should be decided before `0.26.0`**, and the changelog says so
either way.

### Other things B does that the pack did not say

- **`ExplorerTree::loaded_scans`** (new, read-only) so the sidebar reads the rules the scans carried.
- **`run_with_oracle`** on `ExplorerScanRequest`, so a test can count and isolate; `run` passes the production oracle.
- **A floor for an empty directory** is honest: `ignored_entries` now runs the gate *before* answering an empty batch (A's
  follow-up), so an empty directory in a repository the gate refuses is `Floor(GateRefused)`, not "git answered".
- **`ignored` is an ordinary status word** (`explorer-node-entry`'s `$git` selector), before the name like the other five; no
  colour carries it. The i18n completeness scan needed `placement` in `generic_args` for the new key.

### Ablations (committed tree, `ablate.sh`) — ten

Asking about every entry (2 tests); the worker not asking (1, plus two registered load-sensitive shell tests that failed in that
run and are already rows in `test-process-leak.md`); unknown collapsed into not-ignored (2); git's collapse decision ignored (2);
`.git` no longer kept collapsed (1); the display name asked about (1); placement always `AtProjectRoot` (1); the rule line not drawn (1);
the rule sentence long again (2).

### Gate

See the request.
