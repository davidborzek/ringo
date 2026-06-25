# ringo-core

The shared engine behind the [ringo](https://github.com/davidborzek/ringo)
tools: spawning [baresip](https://github.com/baresip/baresip), the `ctrl_tcp`
wire protocol, the call-event model and a phone command abstraction — free of
any TUI or configuration concerns.

It backs the `ringo` softphone and the `ringo-flow` scenario runner. This is an
internal support crate without a stable public API; pin an exact version if you
depend on it directly.

📖 Documentation for the tools: https://davidborzek.github.io/ringo/

Licensed under MIT.
