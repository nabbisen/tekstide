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
project-board-empty-primary-action = Add Project
project-board-empty-secondary-action = Open from path

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
