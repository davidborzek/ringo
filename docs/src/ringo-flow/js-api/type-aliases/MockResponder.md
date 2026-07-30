# Type Alias: MockResponder

> **MockResponder** = [`MockResponseSpec`](../interfaces/MockResponseSpec.md) \| ((`req`) => [`MockResponseSpec`](../interfaces/MockResponseSpec.md))

A static response, or a closure invoked per request (runs on the scenario
 thread, pumped from `until`, so it may close over scenario state).
