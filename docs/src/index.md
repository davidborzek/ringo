<div class="hero">
<h1 class="lockup">
  <img class="lockup-mark" src="logo.svg" alt="" />
  <span class="lockup-text"><span class="lockup-name">ringo</span></span>
</h1>
<p class="tagline">Make and test phone calls from your terminal.</p>
</div>

Three tools that share one engine:

- [**ringo-phone**](ringo-phone/introduction.md) — a terminal softphone: manage SIP
  accounts and place calls without leaving the keyboard.
- [**ringo-flow**](ringo-flow/introduction.md) — a telephony scenario test runner:
  write call flows as JavaScript or TypeScript and run them headlessly in CI.
- [**ringo-mcp**](ringo-mcp/introduction.md) — an MCP server that gives LLM
  agents a telephone: SIP agents as named tools, driven over stdio.

The [source is on GitHub](https://github.com/davidborzek/ringo).

<sub>For tooling/agents: [llms.txt](llms.txt) indexes the docs, and the ringo-flow
scenario API is available as [TypeScript definitions](ringo-flow/ringo-flow.d.ts)
(`.d.ts`). The deprecated Rhai frontend also ships
[`.d.rhai` definitions](ringo-flow/ringo-flow.d.rhai).</sub>
