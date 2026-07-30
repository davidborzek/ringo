# Function: until()

> **until**(`cond`, `within?`): `Promise`\<`any`\>

Resolves with `cond`'s value once it stops throwing, or rejects on timeout.
 `await` it (reads as `await until(...)`); the resolved value lets `.value()` bind a
 verified value. While waiting it yields the event loop, so several `until`/
 `verifyAudio` can run under `await Promise.all([...])`.

## Parameters

### cond

() => `unknown`

### within?

`string`

## Returns

`Promise`\<`any`\>
