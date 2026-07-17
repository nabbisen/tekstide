# tekstide-core

`tekstide-core` contains the core domain, security, project, and content models for Tekstide.

This crate is part of the Tekstide `0.2.0` terminal/runtime/security foundation. It provides the core models used by the `tekstide` workspace application, including:

- ProjectSession and related domain vocabulary;
- Restricted Mode policy/read-model support;
- root-bound project file access;
- bounded explorer state;
- UTF-8 text document buffers;
- safe save and external-change detection;
- TerminalSession lifecycle and project-owned Linux terminal runtime models;
- bounded terminal IO summaries and process-group termination outcomes;
- terminal output security policy, paste classification, and trusted UI spoofing-boundary models.

It is not the desktop GUI, rendered terminal surface, app/UI terminal launcher, AgentRun launcher, transcript/review workflow, command-approval system, or durable audit store. Those areas are deferred after the `0.2.0` foundation release.

Repository documentation:

- Project repository: <https://github.com/nabbisen/tekstide>
- Release scope: <https://github.com/nabbisen/tekstide/blob/main/CHANGELOG.md>
- Future work: <https://github.com/nabbisen/tekstide/blob/main/rfcs/future-work.md>
