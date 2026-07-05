# RFC-003: Information Architecture and UI Mode Model

Status: Proposed  
Target milestone: M0-M1  
Date: 2026-07-04

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- `tekstide-appendix-a-extensibility-plugin-v0.md`
- `tekstide-roadmap-milestones-v0.md`


## Summary

This RFC freezes Tekstide's information architecture: a global Project Board above per-project dual modes. It preserves the original Content Mode and Terminal Immersion Mode, but repositions them as focused surfaces inside an active ProjectSession.

## Motivation

The original dual-mode design is elegant for one project, but Tekstide's main product promise is concurrent multi-project AI CLI workflow. The UI must therefore first answer global questions: what is running, what needs attention, and where should the user go next?

## Goals

- Define top-level navigation.
- Define Project Board content.
- Define active project Content Mode.
- Define Terminal / Agent Immersion Mode.
- Define mode-switching and project-switching semantics.
- Define surface ownership and no-splitting rules for MVP.

## Non-Goals

- Define pixel-perfect styling.
- Define implementation framework widgets.
- Add arbitrary editor splits.
- Add multi-window support.
- Add plugin UI slots.

## Detailed Design

## 1. Top-level application structure

```text
Tekstide Window
├─ Project Board
└─ Active Project Workspace
   ├─ Content Mode
   └─ Terminal / Agent Immersion Mode
```

The Project Board is not a project. It is the global control plane.

## 2. Project Board

The Project Board displays loaded/recent ProjectSessions. Each project row/card should show:

- display name;
- root path hint;
- trust state;
- Git branch/status summary;
- active terminal count;
- active AgentRun count;
- pending approval count;
- review-ready change count;
- dirty file count;
- last activity;
- highest attention state.

Attention states:

| State | Meaning |
|---|---|
| Calm | No action needed. |
| Running | Background work active. |
| Waiting | Approval or user input required. |
| Review | Generated changes or artifacts need review. |
| Failed | Process or agent failed. |
| Risk | Trust/security warning. |

## 3. Active Project Workspace

Only one ProjectSession is active in the main workspace at a time. Background projects continue running and are summarized by the Project Board and global status indicators.

## 4. Content Mode

Content Mode layout:

```text
+-----------------------+-----------------------------------------------+
| Explorer / Project    | One primary content surface                   |
| context               |                                               |
+-----------------------+-----------------------------------------------+
| Status: git, trust, cursor, jobs, approvals, review count          |
+-------------------------------------------------------------------+
```

Allowed main surfaces in MVP:

- text editor;
- Git graph/status surface;
- AgentRun detail;
- diff review;
- handoff/report viewer;
- project trust/settings summary.

MVP rule: no arbitrary split editor panes in Content Mode.

## 5. Terminal / Agent Immersion Mode

Terminal / Agent Immersion Mode hides the explorer and uses the full window for terminal/agent sessions.

Wide layout:

- two vertical terminal panes maximum;
- one primary active session;
- one secondary visible session.

Narrow layout:

- horizontal split or single full-width terminal;
- width preservation is more important than row count.

Hidden terminal sessions continue running and are shown in the bottom multiplexer bar.

## 6. Navigation semantics

Required keyboard actions:

- open Project Board;
- switch active project;
- toggle Content Mode / Terminal Mode;
- cycle visible terminal sessions;
- open current AgentRun detail;
- open pending approval;
- open diff review;
- safe close dialog.

Default keybindings may be refined later, but every primary workflow must be keyboard reachable.

Proposed default posture for v0.1.0:

| Action | Default posture |
|---|---|
| Command Palette | Reserve `Ctrl+Shift+P`. |
| Project Board | Configurable; Linux MVP candidate is `Ctrl+Alt+P`. |
| Mode Toggle | Configurable; platform-specific defaults are allowed. |

`Ctrl+Shift+Esc` must not be selected as the default Project Board shortcut because it conflicts with Windows Task Manager and is too OS-sensitive.

## 7. Surface rules

- A surface belongs to a ProjectSession.
- A surface must not display data from another project unless explicitly labeled as global.
- The UI must never show an approval for one project inside another project's context without clear project labeling.
- The active project can change while background projects continue running.

## 8. Status visibility

Global status should summarize background activity without constant movement. The UI should favor small badges and counts over animations.

Minimum global indicators:

- total active projects;
- total running AgentRuns;
- total pending approvals;
- total review-ready changes;
- highest security warning.

## User Experience Impact

UX principles:

- Calm by default.
- Attention only when action is needed.
- One focused surface at a time.
- Context always visible.
- Managed/supervised/plain modes are visibly distinct.
- Risk decisions are not hidden in terminal output.

## Security and Privacy Impact

Security UI must be contextual and global:

- trust state visible in Project Board and active workspace;
- approval dialogs include project name and root path;
- plain terminal sessions are not displayed as managed AgentRuns;
- Restricted Mode blocks must explain why an action is unavailable.

## Test Plan

- UI state tests for mode switching.
- Snapshot/wireframe tests if framework allows.
- Keyboard navigation tests for Project Board -> project -> AgentRun -> approval -> diff.
- Responsive layout tests for wide/narrow terminal mode.

## Acceptance Criteria

- User can identify which project needs approval from the Project Board.
- User can enter a project and switch between Content and Terminal modes without losing state.
- Terminal Mode never displays more than two visible terminal panes in MVP.
- Content Mode never displays arbitrary multi-editor splits in MVP.
- AgentRun detail and Diff Review are accessible as main content surfaces.

## Risks and Mitigations

- Project Board may become noisy. Mitigation: attention states and calm defaults.
- Users may confuse terminal sessions and AgentRuns. Mitigation: explicit labels and compatibility indicators.

## Open Questions

- Should Project Board use cards or rows as default?
- What is the final default mode-toggle keybinding on each OS?

## Implementation Handoff Checklist

- Implement UI navigation around Project Board first.
- Avoid implementing arbitrary split editing before MVP.
- Add placeholder surfaces before feature-rich implementations.
