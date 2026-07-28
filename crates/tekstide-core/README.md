# tekstide-core

`tekstide-core` contains the core domain, security, project, and content models for Tekstide.

This crate is part of the Tekstide `0.3.0` headless core through RFC-013. It provides the core models used by the `tekstide` workspace application, including:

- ProjectSession and related domain vocabulary, Restricted Mode policy/read-model support, root-bound project file access, bounded explorer state, UTF-8 text document buffers, and safe save with external-change detection;
- TerminalSession lifecycle and project-owned Linux terminal runtime models, bounded terminal IO summaries, and process-group termination outcomes;
- terminal output security policy, paste classification, and trusted UI spoofing-boundary models;
- AI CLI profiles as reviewed launch contracts, with Restricted Mode blocking workspace-local executables, wrappers, project-local `PATH`, and implicit CLI workspace-config discovery;
- AgentRun launch through project-owned terminals, with honest Plain/Supervised/Managed labels and active-file safety before process start;
- bounded local transcript capture with retention limits, per-run opt-out, and purge;
- metadata-only generated-change detection and review-state tracking;
- durable local SQLite audit storage with schema identity, migration harness, corruption diagnostics, restart-safe recovery, and explicit purge.

It is not the desktop GUI, rendered terminal surface, app/UI terminal launcher, or command-approval system. Those areas remain deferred. Durable audit currently records trust decisions, managed AgentRun lifecycle, and blocked root/symlink access only; command approval, paste, restricted-feature, safe-close, configuration-change, transcript-purge, project-added, and plain-terminal producers are defined in the audit schema but not yet wired.

Repository documentation:

- Project repository: <https://github.com/nabbisen/tekstide>
- Release scope: <https://github.com/nabbisen/tekstide/blob/main/CHANGELOG.md>
- Future work: <https://github.com/nabbisen/tekstide/blob/main/rfcs/future-work.md>
