# tekstide-core

`tekstide-core` contains the core domain, security, project, and content models for Tekstide.

This crate is part of the Tekstide `0.1.0` foundation release. It provides the core/shell foundation used by the `tekstide` workspace application, including:

- ProjectSession and related domain vocabulary;
- Restricted Mode policy/read-model support;
- root-bound project file access;
- bounded explorer state;
- UTF-8 text document buffers;
- safe save and external-change detection.

It is not the desktop GUI, PTY terminal runtime, AgentRun launcher, transcript/review workflow, or durable audit store. Those areas are deferred after the `0.1.0` foundation release.

Repository documentation:

- Project repository: <https://github.com/nabbisen/tekstide>
- Release scope: <https://github.com/nabbisen/tekstide/blob/main/CHANGELOG.md>
- Future work: <https://github.com/nabbisen/tekstide/blob/main/rfcs/future-work.md>
