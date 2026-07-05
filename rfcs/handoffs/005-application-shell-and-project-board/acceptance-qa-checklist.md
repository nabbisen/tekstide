# RFC-005 Acceptance / QA Checklist

Source RFC: [RFC-005](../../done/005-application-shell-and-project-board.md)

## Functional Checks

- [ ] First-run empty Project Board appears.
- [ ] User can add at least five projects.
- [ ] Dialog and CLI path flows use the same validation semantics.
- [ ] Non-directory paths are rejected.
- [ ] Permission errors show user-visible errors.
- [ ] Duplicate canonical roots focus the existing ProjectSession.
- [ ] Symlink ambiguity fails closed or requires explicit confirmation.
- [ ] If symlink ambiguity is confirmed, the dialog shows both selected path and canonical target.
- [ ] Project Board uses compact row layout for Linux MVP.
- [ ] User can switch active project.
- [ ] Missing/stale recent projects are labeled and removable by user action.

## State / Persistence Checks

- [ ] New project receives stable persisted `ProjectId`.
- [ ] New root with no valid prior trust becomes `Restricted` before display.
- [ ] Recent projects restore after restart.
- [ ] Corrupt/missing recent-project state does not crash startup.
- [ ] Recent-project state has a version field.
- [ ] Linux recent-project path uses `$XDG_STATE_HOME/tekstide/recent-projects.json` or `~/.local/state/tekstide/recent-projects.json`.
- [ ] Recent-project state is versioned JSON.
- [ ] `last_trust_state_summary` is display-only and does not grant trust.
- [ ] `last_opened_at` and `last_activity` semantics are implemented or documented.
- [ ] UTC timestamps are used internally.

## Security / Privacy Checks

- [ ] Add-project does not auto-load `.env`.
- [ ] Add-project does not auto-start LSP.
- [ ] Add-project does not auto-load workspace plugins.
- [ ] Add-project does not auto-load workspace AI profiles or prompts.
- [ ] Add-project does not run project scripts or hooks.
- [ ] Tekstide itself does not contact network services during add-project.
- [ ] Project Board rendering does not invoke `git`, shell commands, LSPs, hooks, scripts, plugins, AI profile loaders, or network services for placeholder fields.
- [ ] Recent-project state is treated as local-only sensitive path data.
- [ ] Logs/errors do not dump full recent-project state unless user exports diagnostics.

## UX Checks

- [ ] Trust state is visible.
- [ ] Attention state is visible with text or icon+label, not color-only.
- [ ] Placeholder counts are not shown as meaningful zeroes for unimplemented features.
- [ ] Display-state labels include `Permission denied`, `Folder missing`, `Cannot read folder`, `Path changed`, and `Remove from recent` where applicable.
- [ ] `Ctrl+Shift+P` is not used for Project Board.
- [ ] Project Board shortcut is configurable.
- [ ] Sort order follows attention priority, recent activity, recent open, display name.
- [ ] Project Board sort order matches RFC-005, or accepted deviation is documented before RFC-005 completion.

## Safe Removal Checks

- [ ] `CloseAssessment` exists.
- [ ] Idle ProjectSession can be removed.
- [ ] Known active resources route through `NeedsConfirmation`.
- [ ] Unknown active-resource state routes through `UnsupportedOrUnknown` or blocks.
- [ ] No running work, dirty state, or unknown active resource is silently discarded.

## Evidence Required

- [ ] `cargo fmt --check` output, or documented project-specific equivalent.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` output, or documented project-specific equivalent.
- [ ] `cargo test --all-targets` output, or documented project-specific equivalent.
- [ ] Manual or screenshot evidence for first-run empty board.
- [ ] Evidence for five projects added.
- [ ] Evidence for duplicate root handling.
- [ ] Evidence for Restricted badge.
- [ ] Evidence for restart restoring recent projects.
- [ ] Evidence for corrupt state recovery.
- [ ] Test fixture or manual evidence using fake workspace files such as `.env`, `.git/hooks/pre-commit`, and workspace plugin/profile paths.
- [ ] Security note confirming no workspace automation, LSP/process startup, or network behavior on add-project.
