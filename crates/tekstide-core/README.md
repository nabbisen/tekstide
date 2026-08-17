# tekstide-core

`tekstide-core` contains the core domain, security, project, and content models for Tekstide.

This crate is the headless core whose scope boundary was set by RFC-013; the desktop GUI, rendered terminal surface, and rendered dialogs it backs live in the sibling `tekstide` crate, not here. It provides the core models used by that application, including:

- ProjectSession and related domain vocabulary, Restricted Mode policy/read-model support, root-bound project file access, bounded explorer state, UTF-8 text document buffers, and safe save with external-change detection;
- TerminalSession lifecycle and project-owned Linux terminal runtime models, bounded terminal IO summaries, and process-group termination outcomes;
- terminal output security policy (RFC-009), paste classification, and trusted UI spoofing-boundary models — rendered as a real terminal surface and a real paste-confirmation dialog by the `tekstide` crate (RFC-017/RFC-018);
- AI CLI profiles as reviewed launch contracts, with Restricted Mode blocking workspace-local executables, wrappers, project-local `PATH`, and implicit CLI workspace-config discovery;
- AgentRun launch through project-owned terminals, with honest Plain/Supervised/Managed labels and active-file safety before process start;
- bounded local transcript capture with retention limits, per-run opt-out, and purge;
- metadata-only generated-change detection and review-state tracking;
- durable local SQLite audit storage with schema identity, migration harness, corruption diagnostics, restart-safe recovery, and explicit purge.

This crate does not itself contain the desktop GUI, the rendered terminal surface, the app/UI terminal launcher, or rendered command-approval dialogs — those are the `tekstide` crate's. Durable audit currently records trust decisions (including workspace-trust grants and revocations made by a real user, as of `0.10.0`), managed AgentRun lifecycle, blocked root/symlink access, audit-store recovery outcomes, plain-terminal session starts and terminations, paste refusals, and command-approval decisions; restricted-feature, safe-close, configuration-change, transcript-purge, and project-added producers are defined in the audit schema but not yet wired.

Repository documentation:

- Project repository: <https://github.com/nabbisen/tekstide>
- Release scope: <https://github.com/nabbisen/tekstide/blob/main/CHANGELOG.md>
- Future work: <https://github.com/nabbisen/tekstide/blob/main/rfcs/future-work.md>
