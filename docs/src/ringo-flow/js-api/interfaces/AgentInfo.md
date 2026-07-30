# Interface: AgentInfo

## Properties

### aor

> **aor**: `string`

The agent's address-of-record (`sip:user@domain`).

***

### calls

> **calls**: `number`

Number of active calls on this agent.

***

### name

> **name**: `string`

The agent's name (as passed to `agent(name, …)`).

***

### peer?

> `optional` **peer?**: [`Peer`](Peer.md)

The current call's remote party, if there is a call.

***

### reason?

> `optional` **reason?**: `string`

SIP reason phrase of the last response, if any.

***

### registered

> **registered**: `boolean`

Whether the agent is currently registered.

***

### state

> **state**: [`State`](../enumerations/State.md)

Current call phase (compare against `State.*`).

***

### statusCode?

> `optional` **statusCode?**: `number`

SIP status code of the last response, if any.
