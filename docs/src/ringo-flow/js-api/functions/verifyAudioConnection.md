# Function: verifyAudioConnection()

> **verifyAudioConnection**(`a`, `b`): `Promise`\<`void`\>

Assert two-way audio between two agents (a→b then b→a); resolves on success.
 Blocking detection runs on the runtime's blocking pool so the JS thread is free.

## Parameters

### a

[`Agent`](../classes/Agent.md)

### b

[`Agent`](../classes/Agent.md)

## Returns

`Promise`\<`void`\>
