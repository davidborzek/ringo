<div class="hero">
<h1 class="lockup">
  <img class="lockup-mark" src="logo.svg" alt="" />
  <span class="lockup-text"><span class="lockup-name">ringo</span></span>
</h1>
<p class="tagline">Make and test phone calls from your terminal.</p>
</div>

Two tools that share one engine:

- [**ringo-phone**](ringo-phone/introduction.md) — a terminal softphone: manage SIP
  accounts and place calls without leaving the keyboard.
- [**ringo-flow**](ringo-flow/introduction.md) — a telephony scenario test runner:
  write call flows as Rhai scripts and run them headlessly in CI.

The [source is on GitHub](https://github.com/davidborzek/ringo).

<sub>For tooling/agents: [llms.txt](llms.txt) indexes the docs, and the ringo-flow
scenario API is available as [Rhai type definitions](ringo-flow/ringo-flow.d.rhai)
(`.d.rhai`).</sub>
