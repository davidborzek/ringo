# ringo-agent

Runs a SIP user agent as its own process, driven over a line-delimited JSON
(NDJSON) stdio protocol. Part of the
[ringo](https://github.com/davidborzek/ringo) tools, built on `ringo-core`.

A consumer drives an agent through `ProcessClient`, which spawns a child that
re-execs the host binary's `agent` subcommand (wired to `ringo_agent::worker::run`).
Each agent is its own process — its own SIP socket and media stack — so several
can run side by side in isolation. It backs the `ringo-flow` scenario runner.

This is an internal support crate without a stable public API; pin an exact
version if you depend on it directly.

📖 Documentation for the tools: https://davidborzek.github.io/ringo/

Licensed under MIT.
