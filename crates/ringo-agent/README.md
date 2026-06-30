# ringo-agent

Runs a SIP user agent as its own process, driven over a framed stdio protocol
(length-prefixed JSON control frames interleaved with raw PCM audio). Part of the
[ringo](https://github.com/davidborzek/ringo) tools, built on `ringo-core`.

A consumer drives an agent through `ProcessClient`, which spawns a child that
re-execs the host binary's `agent` subcommand (wired to `ringo_agent::worker::run`).
Each agent is its own process — its own SIP socket and media stack — so several
can run side by side in isolation. It backs the `ringo-flow` scenario runner.

Live audio crosses the same stream: received call audio can be streamed out
(`start_rx_audio`, used by `ringo-flow` for tone verification), and PCM can be
streamed into a call (`start_tx_audio`/`push_tx_audio`) as the building block for
a live producer such as a TTS-driven MCP tool.

This is an internal support crate without a stable public API; pin an exact
version if you depend on it directly. The wire protocol is versioned and the
worker rejects a mismatching parent at the handshake, so a stale binary fails
fast instead of desyncing.

📖 Documentation for the tools: https://davidborzek.github.io/ringo/

Licensed under MIT.
