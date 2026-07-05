# RFC-006: ProjectSession State and File Explorer / Editor Basics — Developer Handoff Pack

Source RFC: [RFC-006](../../proposed/006-projectsession-state-and-file-explorer-editor-basics.md)  
Target milestone: **M3**  
Source RFC status: **Proposed**

## Files

- `implementation-handoff.md` — developer-facing implementation constraints and architecture notes.
- `task-breakdown-pr-plan.md` — recommended PR sequence and review gates.
- `acceptance-qa-checklist.md` — acceptance traceability, QA checklist, and release evidence requirements.

This handoff inherits the source RFC lifecycle state. Under the current four-folder RFC policy, the RFC remains in `proposed/` until implementation evidence moves it to `done/`.

## Source Summary

This RFC implements the basic per-project Content Mode: explorer, file open/edit/save, one main content surface, external change detection, and conservative root-bound file access.
