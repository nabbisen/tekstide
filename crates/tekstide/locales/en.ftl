# RFC-016 PR-016-B: source-locale catalog.
#
# The source locale (`en`) is compiled into the binary (see
# `i18n::catalog`), so a missing or corrupt catalog file on disk can
# never make the application unusable -- this file only needs to exist
# at build time.
#
# Actual translations are content work, not this RFC's job (RFC-016
# §Non-Goals). These keys exist to prove the lookup, locale-selection,
# and fallback machinery works, not to cover a real UI -- RFC-015 has
# not been implemented yet, so there is no real UI to cover. Real
# surfaces add real keys as they are built.

app-title = Tekstide
project-board-title = Project Board

# RFC-016 PR-016-D: proves both pluralization and interpolation through
# one realistic key, modeled on `shell::render_text`'s real "blocked
# automation: N" field (response 123/124's own example of why PR-016-D
# unblocks RFC-015 PR-015-D). The selector doubles as the response's
# design constraint made concrete: `$count` may be a genuine number
# (plural-category selection applies) or one of `CountDisplay`'s three
# non-numeric states, expressed as literal string variants alongside the
# plural ones -- the same interpolation argument, the same key, no
# second API. `CountDisplay::label()` itself is not called from here;
# this key only proves the catalog CAN express all four shapes, per
# response 123/124's explicit "record it, do not solve it in PR-016-D."
#
# `[one]` and `*[other]` deliberately use distinct wording (singular
# "automation" vs. plural "automations"), not the same shape with a
# number spliced in -- response 125 Required 2: identical branches made
# `plural_categories_apply_for_english_too_with_its_simpler_one_other_split`
# pass even with the `[one]` variant deleted entirely, proving nothing
# about plural selection despite the test's name.
blocked-automation-count = { $count ->
    [not_implemented] blocked automation: not implemented
    [unavailable] blocked automation: not available
    [unknown] blocked automation: unknown
    [one] {$count} blocked automation
   *[other] {$count} blocked automations
}

# RFC-015 PR-015-B: `status-bar-summary` covers both the two possible
# routes (a literal-variant selector, exactly `blocked-automation-count`'s
# non-numeric branches) and a genuine plural count in one lookup, per
# the same one-key pattern PR-016-D established.
#
# RFC-015 PR-015-E: the Active Project Workspace's sidebar and main-area
# scaffolding for RFC-017/019/020 -- both zones are catalog-driven
# placeholders; `content-area-placeholder-title`/`-body` (PR-015-B's own
# placeholder, shown when no surface existed at all) are retired, since
# every route now has real scaffolding to show.
sidebar-placeholder-title = Sidebar
main-area-content-mode-placeholder = Content Mode. RFC-019 adds the editor and explorer here.
main-area-terminal-mode-placeholder = Terminal / Agent Immersion Mode. RFC-017 adds the terminal here.

# RFC-040 PR-040-C: `mode_toggle_row`'s own two labels -- each names the
# mode a click switches *to*, not the mode currently showing.
main-area-switch-to-terminal-button = Switch to Terminal
main-area-switch-to-content-button = Switch to Content

status-bar-summary = { $route ->
    [project-board] Project Board
   *[active-project-workspace] Project Workspace
} | { $count ->
    [one] {$count} project
   *[other] {$count} projects
}

# Scaffolding for this slice's own layer-composition screenshot evidence
# only (see `shell.rs`'s module doc) -- not a real dialog. RFC-022 supplies
# real trusted dialogs. PR-015-C made it genuinely dismissible (Tab/
# Shift+Tab cycles Acknowledge/Dismiss, Enter or Escape closes it) so the
# modal-exclusivity and focus-trap properties have something real to
# exercise -- a placeholder that never closed could not prove either one.
layer-demo-modal-title = Layer Composition Demo
layer-demo-modal-body = This placeholder proves the modal layer renders above content, never inside it.
layer-demo-modal-acknowledge = Acknowledge
layer-demo-modal-dismiss = Dismiss
layer-demo-modal-dismiss-hint = Tab/Shift+Tab moves focus; Enter or Escape dismisses.

# RFC-018 PR-018-C: the real paste confirmation dialog. Rendered for
# every `RequiresConfirmation` decision -- today that is always a
# multi-line paste (`classify_paste`'s only path to this decision), so
# the body names line count, not paste class. `$line_count` pluralizes
# the same way `project-board-terminal-count` does. The dialog's own
# preview of the pasted content is untrusted text in trusted chrome
# (`text_safety::quote_untrusted`), rendered directly rather than
# through a catalog message -- the same division `surface::board::row_lines`
# already uses for untrusted project names.
paste-confirm-dialog-title = Confirm Paste
paste-confirm-dialog-body = { $line_count ->
    [one] This paste contains {$line_count} line. Allow it to reach the terminal?
   *[other] This paste contains {$line_count} lines. Allow it to reach the terminal?
}
paste-confirm-dialog-preview-truncated = (preview truncated)
paste-confirm-dialog-accept = Paste
paste-confirm-dialog-reject = Cancel
paste-confirm-dialog-hint = Tab/Shift+Tab moves focus; Enter activates; Escape always cancels.

# RFC-015 PR-015-D: the Project Board surface. Empty-state keys are
# catalog-driven purely from `Option::is_some()` -- no core change
# needed, unlike the CountDisplay/attention keys below, which select on
# a real enum the view model already exposes. Never used to render
# `tekstide-core`'s own pre-baked English for these three strings
# (`ProjectBoardEmptyState`'s fields exist but are not read).
project-board-empty-heading = No projects yet
# 0.12.1: `project-board-empty-primary-action` ("Add Project") and
# `-secondary-action` ("Open from path") were rendered as plain `text()`
# widgets naming two actions that do not exist -- there is no in-app way
# to add a project, and both labels were inert from the day they landed.
# Replaced with what is actually true. The keys are gone rather than
# reworded so nothing can render them again by accident; the matching
# fields on `tekstide-core`'s `ProjectBoardEmptyState` still hold the old
# pre-baked English and are still never read (RFC-038 owns removing them).
#
# Release gate finding, 0.13.0: reworded from an imperative primary
# instruction ("To open a project, start Tekstide with its path:") to an
# explicit alternative, now that it renders *after* the path field and
# Browse button rather than before them -- see `board.rs`'s own comment
# at this key's call site.
project-board-empty-open-a-project = You can also open a project from the command line:
project-board-empty-command-example = tekstide /path/to/project
# RFC-038 PR-038-A: the field itself. Focused on arrival at an empty
# board -- see `board.rs`'s own doc for why this is not an
# `iced::widget::text_input` (this project routes every keystroke
# through one reviewed router; a second, widget-internal capture path
# would bypass it).
# RFC-038 PR-038-B: renamed from `project-board-empty-path-field-label` --
# `path_field_section` now renders this on the populated board too
# (`Ctrl+Alt+O`, the second-project case), where "empty" no longer
# describes the board it's showing on.
project-board-path-field-label = Type a project path and press Enter (Ctrl+V to paste):

# RFC-038 PR-038-G: the folder browser's own visible control -- a real,
# clickable button, not only the `Ctrl+Alt+B` accelerator, per D1's
# overturn (a typed path is not an acceptable primary way to choose a
# folder).
project-board-browse-button = Browse...

# RFC-038 PR-038-D: the one-key-reopen button on a remembered-but-closed
# project's own row.
project-board-recent-open-button = Open

# RFC-039 D1: the tab strip's own permanent leftmost entry, always
# present -- the visible route back to the Project Board (workflow 5),
# alongside the pre-existing `Ctrl+Alt+P` accelerator.
project-tab-strip-home = Projects

# RFC-039 PR-039-C: the close control on every project tab -- never on
# the permanent home tab above, which is not a project to close. A
# symbol, not a word, so it reads the same regardless of locale; still
# routed through the catalog rather than a Rust literal, the same
# "nothing user-visible bypasses en.ftl" discipline every other tab
# strip label already follows.
project-tab-strip-close = ×

# RFC-038 PR-038-A: `$reason` is a compile-time symbol
# (`PathFieldError`'s own shape), the same division of labour
# `terminal-launch-refused` already uses -- never the error's Rust
# `Debug` text. `$path` is the user's own typed/pasted text, bounded and
# escaped (`shell::path_field_error_text`) before it ever reaches this
# key, per `what-a-path-field-must-not-trust.md` §1/§3.
project-board-path-field-error = { $reason ->
    [does-not-exist] Couldn't open { $path } — that folder doesn't exist.
    [not-directory] Couldn't open { $path } — that isn't a folder.
    [permission-denied] Couldn't open { $path } — permission denied.
    [cannot-read-folder] Couldn't open { $path } — the folder couldn't be read.
   *[symlink-ambiguous] Couldn't open { $path } — its real location through a symlink is ambiguous.
}

# Every `project-board-*-count` key below shares `blocked-automation-count`'s
# vocabulary for `CountDisplay`'s three non-numeric states
# (`not_implemented`/`unavailable`/`unknown`) -- learned once, reused
# everywhere a `CountDisplay` needs a selector. `CountDisplay::label()` is
# never called for any of these (response 130's explicit decision,
# `surface/board.rs`'s module doc) -- these keys are the alternative
# that makes "never render Unavailable/NotImplemented as 0" a property
# of real CLDR plural selection, not a string comparison to `label()`'s
# output.
project-board-branch-status = { $status ->
    [not_implemented] branch: not implemented
    [unavailable] branch: not available
    [unknown] branch: unknown
   *[other] branch: {$status}
}

project-board-terminal-count = { $count ->
    [not_implemented] terminals: not implemented
    [unavailable] terminals: not available
    [unknown] terminals: unknown
    [one] {$count} terminal
   *[other] {$count} terminals
}

project-board-agent-run-count = { $count ->
    [not_implemented] agent runs: not implemented
    [unavailable] agent runs: not available
    [unknown] agent runs: unknown
    [one] {$count} agent run
   *[other] {$count} agent runs
}

project-board-approval-count = { $count ->
    [not_implemented] approvals: not implemented
    [unavailable] approvals: not available
    [unknown] approvals: unknown
    [one] {$count} pending approval
   *[other] {$count} pending approvals
}

project-board-review-count = { $count ->
    [not_implemented] reviews: not implemented
    [unavailable] reviews: not available
    [unknown] reviews: unknown
    [one] {$count} review
   *[other] {$count} reviews
}

project-board-dirty-file-count = { $count ->
    [not_implemented] dirty files: not implemented
    [unavailable] dirty files: not available
    [unknown] dirty files: unknown
    [one] {$count} dirty file
   *[other] {$count} dirty files
}

project-board-attention = { $attention ->
    [risk] Risk
    [approval_needed] Approval needed
    [review] Review
    [failed] Failed
    [running] Running
    [dirty] Dirty
   *[calm] Calm
}

# RFC-017 PR-017-E: response 150 Required -- `session_bar.rs`'s entries
# were hardcoded English (`slot_label`/`status_label`), the same shape
# `CountDisplay::label()`/`AttentionState::label()` are banned from this
# crate for. One key, two symbol selectors (`$slot`, `$status`) plus a
# genuine number (`$number`) -- the same one-message-one-lookup pattern
# `status-bar-summary`/`project-board-attention` already use, not a
# string built by concatenating three separately-resolved lookups.
session-bar-entry = Terminal { $number } ({ $slot ->
    [primary] Primary
    [secondary] Secondary
   *[hidden] Hidden
}) — { $status ->
    [starting] Starting
    [running] Running
    [exited] Exited
    [failed] Failed
    [terminating] Terminating
   *[unknown] Unknown
}

# RFC-040 PR-040-C, D2: "terminal launch on the workspace" -- always
# shown in Terminal Immersion, panes empty or not; `terminal-launch-refused`
# above already renders the honest reason when the limit is reached, so
# this button is never hidden, only sometimes followed by that notice.
terminal-workspace-launch-button = + New Terminal

# Terminal launch UX handoff: "refusal must be a typed error the shell
# can render... the user pressed a key and is owed a visible answer."
# One message, `$reason` a compile-time symbol
# (`TerminalLaunchRefusal`'s own shape, never the refusal's Rust
# `Debug` text), matching `session-bar-entry`'s own pattern rather than
# a hardcoded shell-local string.
terminal-launch-refused = { $reason ->
    [limit] Terminal limit reached ({ $limit } open) — close one to open another.
   *[error] Couldn't start a terminal.
}

# RFC-022 PR-022-D: same shape as `terminal-launch-refused` above.
# `not-found` is the common first-run state (no AI CLI installed where
# this profile looks) -- an honest, useful message, not a bug to route
# around (response 218). `workspace-blocked` is the correctly-refused
# case for an untrusted project (the profile may discover workspace
# files, so a Restricted project refuses until trust is granted).
agent-run-launch-refused = { $reason ->
    [limit] Agent run limit reached ({ $limit } running) — close one to start another.
    [not-found] No AI CLI found. Install one and try again.
    [workspace-blocked] This project isn't trusted yet — grant trust to start an agent run.
   *[error] Couldn't start an agent run.
}

# RFC-018 PR-018-B: same shape as `terminal-launch-refused` above --
# `$reason` a compile-time symbol (`TerminalPasteRefusal`'s own shape),
# never the refusal's Rust `Debug` text or the pasted content itself.
# `multiline` is structurally unreachable as of PR-018-C (a multi-line
# paste now opens the confirmation dialog instead of being refused) --
# kept only because `TerminalInputDecisionReason` is matched
# exhaustively; the text stays generic rather than naming a state that
# can no longer occur.
terminal-paste-refused = { $reason ->
    [multiline] Multi-line paste blocked.
    [control] Paste blocked: it contains control characters.
    [wrong-target] Paste blocked: the target terminal changed.
    [too-large] Paste blocked: larger than 256 KiB.
   *[trusted-ui] Paste blocked while a dialog is open.
}

# RFC-019 PR-019-B: the explorer tree is trusted chrome, so `$name` is
# untrusted text -- escaped via `text_safety::quote_untrusted` before it
# ever reaches `CatalogArgs::untrusted`, never the raw file name. `$kind`,
# `$state`, and `$symlink` are compile-time symbols from
# `ExplorerNodeKind`/`ExplorerNodeState`/`FileAccessSymlinkStatus` --
# never the enum's own `Debug` text or `tekstide-core`'s hardcoded-English
# label functions (`explorer_node_kind_label` and friends), which is
# exactly what this one message replaces. One lookup, four selectors,
# matching `session-bar-entry`'s own shape rather than concatenating
# separately-resolved strings.
explorer-node-entry = { $kind ->
    [directory] [DIR]
    [other] [OTHER]
   *[file] [FILE]
} { $name }{ $state ->
    [collapsed] {" (collapsed)"}
    [blocked] {" (blocked)"}
    [unreadable] {" (unreadable)"}
   *[available] {""}
}{ $symlink ->
    [in-root] {" [symlink]"}
    [unresolved] {" [broken symlink]"}
    [escapes-root] {" [symlink escapes root]"}
   *[none] {""}
}

explorer-parent-entry = [UP] ..

explorer-empty = This directory is empty.

explorer-truncated-notice = Listing truncated — not all entries are shown.

# `$message` here is `ProjectExplorerStatus::Error`'s own message
# (`ExplorerScanError`'s `Display`), which embeds the target's relative
# path -- attacker-influenced, the same class the node names above are.
# Escaped via `text_safety::quote_untrusted` before it reaches
# `CatalogArgs::untrusted`, same as `$name`.
explorer-status-error = Explorer error: { $message }

# RFC-038 PR-038-G: the folder browser -- `browse-node-entry` is
# `explorer-node-entry`'s own shape, narrowed: no `$kind` selector
# (every row is a directory by construction, `DirectoryBrowseScan`
# never lists anything else) and no `$symlink` selector (`BrowseNode`
# carries no symlink-status field at all -- see its own doc comment for
# why browsing follows symlinks rather than tracking them). `$name` is
# untrusted, escaped via `text_safety::quote_untrusted` before it
# reaches `CatalogArgs::untrusted`, same discipline as the project
# explorer's own node names.
browse-node-entry = { $name }{ $state ->
    [collapsed] {" (collapsed)"}
    [unreadable] {" (unreadable)"}
   *[available] {""}
}

browse-parent-entry = [UP] ..

browse-empty = This directory has no subdirectories.

browse-truncated-notice = Listing truncated — not all entries are shown.

browse-dialog-title = Choose a project folder
# `$path` is `DirectoryBrowseScan::current_dir` -- a real, canonical
# filesystem path, untrusted exactly as a node name is (escaped by the
# caller, `surface::explorer::browse_tree_lines`, before this key ever
# sees it).
browse-dialog-current = Current: { $path }
browse-dialog-hint = Enter opens a folder; Space chooses the folder shown above; Escape cancels.
browse-navigate-error = Couldn't open that folder.
# RFC-040 PR-040-B: the real, clickable equivalent of `Space` -- commits
# `browse-dialog-current`'s own folder, not whichever row is highlighted
# (that is what a row click does instead, `Enter`'s own mouse equivalent).
browse-dialog-choose-button = Open this folder

# RFC-019 PR-019-C: the editor's chrome header -- everything here is
# chrome (RFC-016's editor exception applies only to the text area
# itself), so `$path` is untrusted and escaped the same way an explorer
# node name is. `$state` replaces `text_document_state_label` (the fourth
# named hardcoded-English producer) -- a compile-time symbol from
# `TextDocumentState`, never the label function's own English word.
editor-chrome = { $path }{ $state ->
    [dirty] {" (unsaved changes)"}
    [external-changed] {" (changed on disk)"}
    [conflict] {" (conflict)"}
    [save-error] {" (save error)"}
   *[clean] {""}
}

# RFC-006 Amendment 1 / RFC-019 PR-019-D: the cursor's own position,
# 1-indexed for display -- `TextCursor` itself is 0-indexed internally,
# matching every other zero-based offset in this crate. Trusted output
# only (two numbers); nothing here is attacker-influenced.
editor-cursor = Line {$line}, Column {$column}

# RFC-040 PR-040-C, D2: "save on the editor" -- shown whenever a
# document is open, regardless of dirty state, matching `Ctrl+S`'s own
# always-available behaviour (`attempt_save_active_document` is a safe
# no-op-shaped call on a clean document, not an error).
editor-save-button = Save

editor-empty = No file is open. Select a file in the explorer and press Enter.

# `$message` is `TextDocumentOpenError`'s own `Display`, which -- like
# `ExplorerScanError`'s -- embeds the target's relative path in every
# variant, including the 4 MiB `TooLarge` refusal this message renders.
# Escaped before it reaches the catalog, the same finding PR-019-B made
# for `explorer-status-error` applied here before it needed catching in
# review a second time.
editor-open-error = Could not open file: { $message }

# RFC-019 PR-019-D: the conflict dialog `TextDocument::save()`'s
# unconditional `BlockedExternalChange` refusal renders into --
# `save()` has no force-overwrite bypass, so this dialog is the only way
# past a conflict. `$path` is the same attacker-influenced, escaped path
# `editor-chrome`'s header already shows. Reload re-opens the file fresh
# (discarding local edits, taking disk's current content); Dismiss/
# Escape leaves the file untouched -- mirrors `paste-confirm-dialog-*`'s
# shape exactly.
#
# RFC-019 PR-019-E: `ProjectContentStatus::Conflict` (this modal's own
# trigger) is set for two different real situations --
# `TextDocument::save()`'s own `block_external_change` sets the
# document's state to `Conflict` only when the buffer was actually
# dirty, and to `ExternalChanged` otherwise (nothing local to lose).
# `$reason` selects between them so this dialog never claims "your local
# changes will be discarded" when there were none -- found live during
# closeout (`content status: conflict | document: external changed |
# dirty files: 0`, a real save on a real clean-but-externally-changed
# document), not merely reasoned about.
external-change-dialog-title = File Changed On Disk
external-change-dialog-body = { $path } changed on disk since it was opened. { $reason ->
    [conflict] Reload to see the new content (your local changes will be discarded), or dismiss to keep editing without saving.
   *[external-changed] Reload to see the new content, or dismiss to keep your current view without saving.
}
external-change-dialog-reload = Reload
external-change-dialog-dismiss = Dismiss
external-change-dialog-hint = Tab/Shift+Tab moves focus; Enter activates; Escape always dismisses.

# RFC-022 PR-022-E: `$command` is `ApprovalRequest.display_command`,
# already escaped by the model (RFC-021's ten-probe suite) and
# isolation-wrapped here; `$cwd` is `ApprovalRequest.cwd`, raw from the
# adapter and escaped for the first time at this widget (response 221 --
# it is the sharper attack surface, since a user skims the directory to
# confirm context rather than reading it as carefully as the command).
# `$risk` is Tekstide's own classification, never adapter text, so it
# needs no escaping.
approval-dialog-title = Command Approval Requested
approval-dialog-body = An AI CLI is asking to run:
    { $command }
    in { $cwd }.
    Risk: { $risk ->
        [low] Low
        [medium] Medium
        [high] High
       *[destructive] Destructive
    }
# what-the-dialog-must-not-lie-about.md §2: "the highest-consequence
# sentence in this RFC." States plainly, in the words a user actually
# reads (not only in documentation), the three things this dialog must
# never let a user assume by omission: that a decision here is
# enforced, that approving means the command is safe, and that the
# command shown is all the adapter will do. Response 222: the third
# non-claim does not depend on open question 3 (RFC-022's interrupt-
# timing question, still open as of this text) -- it is about what a
# single dialog's authority covers, true whenever the dialog appears,
# regardless of how or when it was reached.
approval-dialog-cooperative-notice = This choice is advisory, not a safeguard: Tekstide sends it to the AI CLI, but the AI CLI decides whether to actually run the command. Approving does not make the command safe, and rejecting cannot stop the AI CLI from running it anyway. This is also only one request -- the AI CLI may make others, with or without asking.
approval-dialog-approve = Approve Once
approval-dialog-reject = Reject
# Response 222: matches RFC-018's paste dialog ("Escape always cancels"),
# not `external-change-dialog-hint`'s "dismisses" -- and deliberately
# does not say what Escape leaves behind. The previous wording ("leaves
# this request pending") committed to a state open question 3 has not
# decided exists: if that question resolves toward interrupt-on-arrival,
# a request Escape leaves pending has to do *something* next (reappear,
# queue, expire) that is not yet designed. Provisional until 220 answers
# it -- do not extend this hint to describe outcomes this line does not
# yet know about.
approval-dialog-hint = Tab/Shift+Tab moves focus; Enter activates; Escape always cancels.

# RFC-022 PR-022-E ("the arrival model"), response 233: the queue-viewing
# surface (`ProjectOpenSurface::ApprovalHistory`). Renders every retained
# `ApprovalRequest` for the active project -- decided and expired
# included, not only ones still awaiting a decision, so the two notices
# below are non-optional, not cosmetic.
approval-history-heading = Approval History
# The retention-limit disclosure: this list shows the most recently
# retained requests, not the project's complete history -- deliberately
# unnumbered (no `$limit` interpolation) so the copy stays true whether
# or not `approval_history_limit` is configured, rather than branching
# on an `Option<u32>` the render layer would otherwise have to unwrap.
approval-history-retention-notice = This list shows the most recently retained requests. Older entries may already have been removed to stay within this project's retention limit -- this is not necessarily the complete history.
# The classifier-limitation disclosure (task-breakdown-pr-plan.md's own
# non-optional item): risk level is Tekstide's own inference from the
# command's argv, not a guarantee. An unclassified or misclassified
# command can still be destructive.
approval-history-classifier-notice = Risk level is Tekstide's own automatic classification of the command, not a guarantee -- an unrecognized or misclassified command can still be destructive. Read the command itself, not only its risk label.
approval-history-empty = No approval requests recorded for this project yet.
# `$command`/`$cwd`/`$risk` reuse `approval-dialog-body`'s own escaping
# and selector conventions exactly (untrusted, quoted before this point;
# risk is Tekstide's own classification, never adapter text). `$state`
# is this surface's own addition -- distinguishing "still Pending and
# answerable" from "still Pending but expired" is the entire reason
# response 231/RFC-022 requires this surface to exist at all ("visibly
# unanswerable, not merely fail when acted on").
approval-history-entry = { $command }
    in { $cwd }
    Risk: { $risk ->
        [low] Low
        [medium] Medium
        [high] High
       *[destructive] Destructive
    }
    Status: { $state ->
        [answerable] Awaiting your decision
        [expired] Expired -- no longer answerable
        [approved] Approved once
        [rejected] Rejected
        [edited-and-approved] Edited and approved
       *[unknown] Unknown
    }
approval-history-entry-open = Open

# RFC-040 PR-040-C, D2: `top_bar_actions_row`'s own two labels -- global
# actions, in the chrome on every route.
top-bar-trust-settings-button = Trust Settings
top-bar-help-button = ?

# RFC-032: `ProjectOpenSurface::TrustSettings`'s own view. `$state` is a
# compile-time literal symbol (`trust_state_symbol`), never
# `WorkspaceTrust`'s own `Debug` text.
trust-settings-empty = No active project.
trust-settings-heading = Workspace Trust
trust-settings-current-state = Current state: { $state ->
    [trusted] Trusted
    [revoked] Revoked (not currently trusted)
   *[restricted] Restricted
}
trust-settings-grant-button = Grant Trust…
trust-settings-revoke-button = Revoke Trust

# RFC-033 PR-033-B: "for future runs" is load-bearing, not decoration --
# what-purge-must-remove.md requires declining future capture to never
# read as deleting transcripts that already exist. This setting is
# per-project and persists across a restart, independent of trust
# state.
trust-settings-capture-current-state = Transcript capture: { $state ->
    [declined] Off for future runs
   *[enabled] On
}
trust-settings-capture-decline-button = Decline Future Capture
trust-settings-capture-allow-button = Allow Future Capture

# RFC-033 PR-033-C: `transcript_local_data_summary`'s own framing --
# "a user deciding whether to purge needs to see what is retained."
# `$bytes` has no plural machinery, matching `agent-run-detail-window-full`'s
# own precedent for a byte count elsewhere in this file; `$count`
# (transcripts) does, matching every other item-count key here (e.g.
# `project-board-dirty-file-count`).
trust-settings-retained-transcripts = Retained locally: { $count ->
    [one] {$count} transcript
   *[other] {$count} transcripts
} ({ $bytes } bytes)
trust-settings-purge-button = Purge Project Transcripts…

# RFC-040 PR-040-C, D2: "agent run where a trusted project's actions
# live" -- always shown, not hidden while Restricted: `Ctrl+Alt+A`
# already allows the attempt and shows the real refusal notice when
# untrusted, so the button reuses that same honest, already-tested
# behaviour rather than a second, silent-when-untrusted path.
trust-settings-launch-agent-run-button = Launch AI CLI Run
trust-settings-agent-run-report-button = AgentRun Report
trust-settings-approval-history-button = Approval History

# PR-020-B: `ProjectOpenSurface::AgentRunDetail`'s own view -- the most
# recently launched run in the active project (this slice's own answer
# to "which run is current"), rendering its transcript through
# `read_window`, escaped at the widget (`the-window-boundary.md` §2).
# The transcript body itself is not a Fluent message: it is rendered
# directly from `text_safety::quote_untrusted`'s escaped output
# (`agent_run_detail_transcript_body`), the same "raw content, not a
# translatable sentence" treatment the editor's own document body
# already gets -- only the chrome around it (this block's keys) goes
# through the catalog.
agent-run-detail-empty = No active project.
agent-run-detail-no-runs = No agent run in this project yet.
agent-run-detail-heading = AgentRun Report
agent-run-detail-no-transcript = No transcript is available for this run.
agent-run-detail-read-error = The transcript for this run could not be read.
# D5: `Complete` vs `StillBeingWritten`, in the type -- rendered as two
# distinct messages, never flattened into one "status" string with a
# boolean behind it.
agent-run-detail-status-active = This run is still active. The transcript below may still be growing.
agent-run-detail-status-finished = This run has finished. The transcript below is complete.
# D2: the **reader window** notice -- this is a tail slice of a
# possibly-larger file, not the writer's own truncation (see
# `agent-run-detail-writer-truncated` below, a separate and independent
# fact). `$delivered_start` is the *delivered* offset
# (`the-window-boundary.md` §1's own required report), which can differ
# from what was requested when resynchronization moved past a token
# straddling the raw boundary.
agent-run-detail-window-full = Showing the complete transcript ({ $total_len } bytes).
agent-run-detail-window-partial = Showing the most recent { $shown_len } bytes of a { $total_len } byte transcript, starting at byte { $delivered_start }.
# The **writer truncation** notice -- independent of the reader-window
# notice above. This means RFC-011's bounded writer itself stopped
# capturing before this run's real output ended; some of what the run
# actually produced was never saved anywhere, which no reader window
# size could recover.
agent-run-detail-writer-truncated = This transcript's own storage was truncated while it was being captured -- some of what this run produced was never saved, independent of the window shown above.

# RFC-020, the change review surface (`change-review-surface.md`): renders
# `ChangeSet::bounded_summary`, the most recent change set for the active
# project. Read-only -- RFC-034's own job is acting on one.
change-review-empty = No active project.
change-review-no-changes-yet = No changes have been detected in this project yet.
change-review-heading = Change Review
# The RFC's own required, non-optional disclosure -- stated on the
# surface itself, not only in documentation, every time this surface
# renders, not only when something was actually excluded.
#
# RFC-035 PR-035-A closeout: corrected from "excludes .git/, target/,
# and node_modules/" -- that was true when RFC-020 shipped and stopped
# being true the moment .git/hooks/ and .git/config became watched.
# This is the exact "shipped statement this slice falsifies" the
# handoff's own closeout section names.
change-review-disclosure = Detected changes only, not all changes: detection is metadata-only and conservative, and excludes target/ and node_modules/ by design. Most of .git/ is excluded too -- only .git/hooks/ and .git/config are watched, since those are the two places a change could install or redirect code that runs on your machine. This is not a review, an approval, or a claim that a change is safe.
# `$status` is a compile-time literal symbol (`change_detection_status_symbol`),
# never `ChangeDetectionStatus`'s own `Debug` text. A truncated *scan*
# (`partial`) is a distinct fact from a *display* limit
# (`change-review-omitted-files` below) -- this line is never allowed to
# collapse into "nothing changed" the way `what-a-clickable-modal-must-not-become.md`'s
# own sibling documents already require elsewhere in this crate for a
# different kind of truncation.
change-review-detection-status = Detection: { $status ->
    [unavailable] Unavailable -- change detection could not run for this project
    [unsupported] Unsupported -- this project's detection source does not support this operation
    [partial] Partial -- the scan itself was bounded at { $limit } entries; more changes may exist and were never counted
    [failed] Failed -- { $reason }
   *[complete] Complete
}
change-review-detection-failure-cross-project-baseline = the baseline snapshot belonged to a different project
change-review-detection-failure-metadata-read-failed = a file's metadata could not be read
change-review-detection-failure-path-outside-root = a detected path resolved outside the project root
change-review-detection-failure-root-unavailable = the project root was not available to scan
# `$count` is `ChangeSetSummary::changed_file_count` -- the real total,
# independent of how many of `shown_changed_files` this surface actually
# lists below.
change-review-file-count = { $count ->
    [one] { $count } file changed
   *[other] { $count } files changed
}
# `$path` is untrusted -- filesystem-derived, attacker-influenceable,
# escaped through `text_safety::quote_untrusted` before it ever reaches
# this key, the same discipline every other rendered path in this crate
# already uses.
change-review-file-entry = { $path }
# The **display** limit, distinct from `change-review-detection-status`'s
# own `partial` branch above (the **scan** limit) -- `$count` is
# `ChangeSetSummary::omitted_changed_file_count`, always zero or more
# real files that were detected but are not individually listed above.
change-review-omitted-files = { $count ->
    [one] 1 more changed file is not shown above
   *[other] { $count } more changed files are not shown above
}
# RFC-035 PR-035-B, corrected per review response 326: a *third*
# truncation fact -- `$count` is `ChangeSetSummary::changed_files_omitted_by_detection`,
# paths a scan found but never stored at all because the true count
# exceeded `max_changed_paths`. Distinct from `change-review-omitted-files`
# above: that one is recoverable (the paths exist in the model, just past
# a display limit); this one is not (the paths are nowhere), and the
# wording says so rather than reading the same as the recoverable case.
change-review-detection-omitted-files = { $count ->
    [one] 1 further changed file was never recorded at all -- the scan capped how many it would track, and this one cannot be recovered by showing more
   *[other] { $count } further changed files were never recorded at all -- the scan capped how many it would track, and these cannot be recovered by showing more
}
change-review-review-state = Review state: { $state ->
    [accepted] Accepted
    [partially-accepted] Partially accepted
    [rejected] Rejected
    [superseded] Superseded
   *[unreviewed] Unreviewed
}
change-review-open-button = Change Review

# RFC-034: a review decision. `what-a-review-decision-must-not-claim.md` §4's own
# answer to the disclosure-density question -- one combined sentence rather than
# three separate ones, since finality (§3), session-scope (D0), and "no file is
# touched" (§1's falsifiable claim) are, from the reader's own perspective, one
# fact: *this is a note to yourself, for now.* Rendered once, beside the two
# buttons, only while they are offered (D4) -- never after a decision, when there
# is nothing left to disclose about a click that already happened.
change-review-decision-notice = Marking this here only records your own note about it: it changes no file, cannot be undone, and disappears when you close Tekstide.
# D3: a *different* fact from `change-review-content-stale`'s own wording, on
# purpose -- that one is about content read at a passed moment; this one is
# about a decision that is still about what was detected, not what is on disk
# now. Reuses `diff_content_is_stale`'s own check (RFC-024), not a second
# staleness notion. Rendered beside the decision controls, which stay live.
change-review-decision-stale-tree = The files on disk have changed since this change set was detected. You can still record your decision about what was detected.
# D1: exactly these two, from `Unreviewed`/`PartiallyAccepted` only, offering an
# opinion, never `PartiallyAccepted` or `Superseded` -- facts a button press is
# not the source of truth for. "Mark", not "Accept"/"Reject" alone: the label
# itself reads as recording a note, not performing an action, the same "the
# words must carry it, at the point of the click" requirement
# `change-review-decision-notice` above also exists to satisfy, redundantly by
# design.
change-review-accept-button = Mark accepted
change-review-reject-button = Mark rejected

# RFC-041, the change content preview (`what-a-content-preview-must-not-claim.md`).
# `$path` is untrusted -- filesystem-derived, escaped through
# `text_safety::quote_untrusted` before it ever reaches this key, the
# same discipline `change-review-file-entry` already uses.
change-review-content-heading = Preview: { $path }
# §1's own required, non-optional claim -- rendered on the screen,
# beside the content, every time a modified file's *current* content is
# shown. Never true for Added (the whole file is the whole change) or
# for anything else that reaches this surface.
change-review-content-not-a-diff = This is the file's current content, not a diff. It cannot show what changed -- only what the file looks like now. Whether the agent's own edit is still there, was reverted, or was overwritten by something else since, this screen cannot tell you.
change-review-content-deleted = This path was deleted. It was a { $kind ->
    [directory] directory
    [symlink] symlink
    [other] filesystem entry
   *[file] file
} before it was removed -- there is no content to preview.
change-review-content-non-text = This file is not text ({ $len ->
    [one] 1 byte
   *[other] { $len } bytes
}) -- no preview is attempted.
change-review-content-non-file = This path exists but is not a file (it is a { $kind ->
    [directory] directory
    [symlink] symlink
   *[other] filesystem entry
}) -- there is no content to preview.
# D2: refuse rather than render-with-a-warning. "Say which" -- this
# names the reason rather than a generic "cannot show content".
change-review-content-stale = This change set's baseline is no longer authoritative: the file has changed on disk since this preview was opened. Re-select it to see the current content, labelled honestly against what is on disk now.
# RFC-041 D1's own ablation case: retention was dropped (or never
# survived the application closing -- the same in-memory-only
# limitation `agent_run_change_baselines` already discloses). The
# metadata above is unaffected; only content preview is unavailable.
change-review-content-unavailable = Content preview is no longer available for this change set.
change-review-content-error-not-detected = This path is not part of the detected change set.
change-review-content-error-access = This file could not be opened for preview ({ $reason ->
    [absolute-path-not-allowed] an absolute path is not allowed here
    [invalid-relative-path] the path is not valid for this project
    [missing-path] the file no longer exists at this path
    [permission-denied] permission was denied
    [cannot-read-path] the path could not be read
    [root-escape] the path resolves outside the project root
   *[symlink-escape] a symlink in the path resolves outside the project root
}).
change-review-content-error-metadata-unavailable = This file's metadata could not be read.
change-review-content-error-too-large = This file is too large to preview: { $len } bytes exceeds the { $max } byte limit. Refused whole, not shown truncated.
change-review-content-error-read-failed = Reading this file's content failed.
# RFC-042 D3: a distinct refusal from the byte bound above -- a row
# count, not a byte count. Refused whole, never truncated, and never
# worded as a count of omitted *files* (this surface already has two of
# those, for a different thing).
change-review-content-error-too-many-lines = This file has too many lines to preview: { $lines } exceeds the { $max } line limit. Refused whole, not shown truncated.

# RFC-032 `what-the-trust-dialog-must-say.md`: `$path` is the project's
# **canonical** path -- what trust actually binds to
# (`docs/src/contributors/security-decisions.md`) -- escaped via
# `text_safety::quote_untrusted` before it ever reaches
# `CatalogArgs::untrusted`, the same primitive every other untrusted-text
# site in this crate uses. The canonical sentence (§3) is reproduced
# verbatim from the decisions page, not paraphrased; the present-and-
# future consequence (§4) and what revoking does and does not undo (§6)
# are both stated explicitly rather than left to be inferred by
# omission. The nine restricted-mode features are deliberately not
# listed anywhere in this message (§3): nobody weighs a nine-item list
# at a decision point.
trust-grant-dialog-title = Grant Workspace Trust?
trust-grant-dialog-body = { $path }

    Files inside the trusted folder may configure Tekstide and cause programs to run.

    This covers files written to this folder in the future too -- including anything an AI agent run here writes -- for this session and every session after, until you revoke it.

    Revoking stops it from loading again; it does not undo anything that has already run.
# Appended to `trust-grant-dialog-body` only when the project's root
# path (as opened) differs from its canonical path -- `$root_path` is
# escaped the same way `$path` above is.
trust-grant-dialog-symlink-notice = You opened this project at { $root_path }, which resolves to the folder above.
trust-grant-dialog-grant = Grant Trust
trust-grant-dialog-cancel = Cancel
trust-grant-dialog-hint = Tab/Shift+Tab moves focus; Enter activates; Escape always cancels.

# RFC-033 PR-033-C, `what-purge-must-remove.md`: the confirmation must
# name the scope (this project only) and state the count/bytes affected
# -- `$count`/`$bytes` are captured at dialog-open time
# (`TranscriptPurgeModal`'s own doc), the same "captured, not re-read"
# shape `trust-grant-dialog-body` above already uses. Does not claim
# purge removes every trace: a tombstone remains
# (`purge_project_transcripts`'s own real behavior), so this message
# says only what disappears -- the bytes -- not "all data" or similar.
transcript-purge-dialog-title = Purge all transcripts for this project?
transcript-purge-dialog-body = This permanently deletes { $count ->
    [one] {$count} transcript
   *[other] {$count} transcripts
} ({ $bytes } bytes) stored locally for this project. Other projects are not affected. This cannot be undone.
transcript-purge-dialog-purge = Purge
transcript-purge-dialog-cancel = Cancel
transcript-purge-dialog-hint = Tab/Shift+Tab moves focus; Enter activates; Escape always cancels.

# RFC-039 PR-039-C, `what-closing-a-project-must-not-lose.md` §2: names
# the project by its canonical path, not display name alone -- the same
# minimal "{ $path }" shape `trust-grant-dialog-body` above already uses,
# since closing shares that dialog's own reason for needing the real
# path: a wrong belief here becomes a destructive action against the
# wrong target, unlike switching.
project-close-dialog-title = Close this project?
project-close-dialog-body = { $path }
# §1: prefixes the real counts `assess_project_close` computed
# ("2 running processes, 1 dirty file"), appended in Rust rather than
# through this template -- those counts are trusted but not yet
# catalog-driven text, the same disclosed limitation
# `surface::board`'s own `trust_label`/`availability_label` already have.
project-close-dialog-live-work-prefix = This will end:
project-close-dialog-close = Close
project-close-dialog-cancel = Cancel
project-close-dialog-hint = Tab/Shift+Tab moves focus; Enter activates; Escape always cancels.


# 0.12.1: descriptions for `keyboard_help::keyboard_help_lines`, one per
# live `KeybindingPolicy` rule. Before this release the string `Ctrl`
# appeared zero times in this catalogue while nine bindings were live, so
# every capability the product had was reachable only by reading
# `navigation.rs`. Each key is looked up from an exhaustive match in
# `keyboard_help.rs`, so a new user-visible action cannot compile without
# one; a *missing* key is caught by
# `every_live_binding_is_described_to_the_user`, since `Catalog::get`
# falls back to echoing the key rather than failing.
#
# Each description says what the action does AND what it needs, because
# the commonest way to conclude this product is broken is to press a key
# whose precondition is not met and watch nothing happen.
keyboard-help-open-project-board = Project Board
keyboard-help-open-project-entry-field = Add a project by path
keyboard-help-toggle-project-mode = Switch between Content and Terminal (needs an open project)
keyboard-help-launch-terminal = New terminal (needs an open project)
keyboard-help-paste-into-terminal = Paste into the focused terminal
keyboard-help-save-active-document = Save the open file
keyboard-help-launch-agent-run = Launch an AI CLI run (needs a trusted project)
keyboard-help-open-current-agent-run-detail = AgentRun Report for the latest run
keyboard-help-open-approval-history = Approval History (needs an open project)
keyboard-help-open-trust-settings = Trust Settings: grant trust, transcript capture and purge
keyboard-help-open-diff-review = Change Review: what the most recent agent run changed
keyboard-help-open-help = This list
keyboard-help-open-folder-browser = Browse for a project folder
keyboard-help-switch-active-project = Switch to the next open project

# RFC-038 PR-038-C: the Help modal itself. Reachable from anywhere;
# replaces the Project Board's own former keyboard list.
help-dialog-title = Keyboard reference
help-dialog-hint = Escape closes this.
# RFC-040 PR-040-B: this modal's own real, clickable close control --
# no decision to make, just a mouse-reachable way out (this dialog had
# zero buttons of any kind until this slice).
help-dialog-close = Close

# 0.12.1: rendered beside `status-bar-summary`, on the same line so the
# status bar's height -- which `content_area_height` subtracts to size
# real terminal panes -- is unchanged. Points at the Project Board
# because that is where the full keyboard list lives when nothing is
# open; it is deliberately not a claim that Ctrl+Alt+P opens "help".
status-bar-key-hint = Ctrl+Alt+P Project Board
