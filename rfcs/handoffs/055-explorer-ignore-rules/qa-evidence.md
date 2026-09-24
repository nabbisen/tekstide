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
2. **The whole gate is not "a few milliseconds"; it is ~115 ms warm (632 ms cold) on this repository, and 99 % of it is
   the attributes walk.** `evaluate_with_environment` = version (cached) + `config --list --null` + `worktree_names_a_content_driver`
   (the gitlink check and a recursive walk of every `.gitattributes`, **237,061 entries in 85 ms**, 117 ms in the gate).
   RFC-055's acceptance measured `--version`, `config`, `rev-parse` and `check-ignore` at ~1 ms each and added them up;
   it did not run `evaluate`. That walk is about whether a **content comparison** can be trusted — the module's own doc
   calls it "not a safety problem" — and `check-ignore` reads no content. So the query asks **only the configuration half**
   (`vet_configuration`, split out of `evaluate` with `evaluate`'s behaviour unchanged: all 43 existing gate tests pass).
   *A call that paid the whole gate would cost ~115 ms per expansion, seven frames.*
3. **A quarter of what remained was the poll interval.** `run_bounded` slept 10 ms between `try_wait`s, so every git call
   cost up to 10 ms more than the process. Measured, 21 entries of this repository's root, the whole call (gate + query):
   **20 ms at 10 ms polling, 2.3 ms at 1 ms** (vet 1.1 ms, query 1.1 ms); a 37-name nested directory 2.3 ms. I changed it to
   1 ms, which also speeds the status-bar refresh's three subprocesses; the diff is one line and a comment. **The number the
   100 000-entry fixture is to be held against in PR-055-B is this whole call, ~2.3 ms, not the query alone.**

### Judgment calls, disclosed

- **Only the configuration half of the gate.** Not what the pack literally says ("exactly as `compute_summary_with_environment`
  does it"), for the reason in finding 2. It is *not* weaker for the danger the gate exists for — a program the configuration
  names — because that is the half it keeps. What it does not repeat is the gitlink check (`R1`, a submodule's own config
  consulted by `git status`): **measured, `check-ignore` does not run a poisoned submodule's filter or fsmonitor**, and
  `a_poisoned_submodule_is_not_run_by_the_query` pins that with the review-406 fixture, so a git that changes it fails a test
  rather than a user. If you would rather have the gitlink check anyway, it is one call; it costs a `git ls-files -s` per
  expansion and refuses every repository with a submodule.
- **`AcceptedBranchOnly` is not enough.** A repository with any config key outside the allowlist (an editor's
  `branch.*.vscode-merge-base`, `include.path`) gets `Unknown(GateRefused)` and, in PR-055-B, the floor list. That is the
  cost of finding 1: I could not allow those keys and still say the program named by `core.fsmonitor` is never run. So the
  acceptance criterion "a repository whose `.gitignore` does not name `target/` shows `target/` as expandable" holds for
  repositories whose configuration is allowlist-clean and falls back to the floor for the others. **B's capture should say which.**
- **A repository at or above `$HOME` is not asked** (`NotARepository`). A dotfiles repository at `~` with `.gitignore` = `*` would
  answer "ignored" for every entry of every project beneath it, and `show_ignored = false` would then hide the user's
  whole tree — the fail-closed outcome the risk document ranks worse. The pack does not mention it; a nested project root
  (which it requires) is what exposes it, because the search now walks *above* the project root. Ablated:
  `a_repository_at_or_above_home_is_not_asked` fails without it. Say if you would rather not have the special case.
- **The repository is the nearest `.git` at or above the canonical directory**, which is what git's own discovery finds. This is
  wider than `compute_summary`, which reads only a `.git` at the project root — so a project *inside* a repository gets ignore
  knowledge while its status bar still says Git is not available. Both are true statements; they are not the same question.
- **`MAX_IGNORE_QUERY_ENTRIES = 1 024`**, four times the explorer's 256, stated again where the subprocess is so no caller can turn
  a scan into an unbounded write. With the 255-byte name cap the reply cannot reach the 1 MiB pipe bound (a `const` assertion),
  so `read_bounded` truncating it — which would turn "ignored" into "not ignored" — is unreachable.

### Gate

See the request. fmt, clippy `-D warnings`, three consecutive runs, fresh `TMPDIR`.
