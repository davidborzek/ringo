# Interface: Agent

## Properties

### headers

> `readonly` **headers**: `Record`\<`string`, `string`\>

All received INVITE headers as a `{ name: value }` object.

***

### peer?

> `readonly` `optional` **peer?**: [`Peer`](Peer.md)

The current call's remote party: `{ uri, number, name }`, or `undefined`.

***

### quality?

> `readonly` `optional` **quality?**: [`CallQuality`](CallQuality.md)

RTP media quality of the active/last call (`{ mos, rtt, jitter, packetLoss }`),
 or `undefined` until metrics are available (no RTCP report yet).

***

### reason?

> `readonly` `optional` **reason?**: `string`

***

### receivedDtmf

> `readonly` **receivedDtmf**: `string`

***

### registered

> `readonly` **registered**: `boolean`

***

### state

> `readonly` **state**: [`State`](../enumerations/State.md)

***

### statusCode?

> `readonly` `optional` **statusCode?**: `number`

## Methods

### abortTransfer()

> **abortTransfer**(): `void`

#### Returns

`void`

***

### accept()

> **accept**(): `void`

#### Returns

`void`

***

### attendedTransfer()

#### Call Signature

> **attendedTransfer**(`target`): `void`

Start an attended transfer to a target: another `Agent` or a URI string.

##### Parameters

###### target

`Agent`

##### Returns

`void`

#### Call Signature

> **attendedTransfer**(`target`): `void`

##### Parameters

###### target

`string`

##### Returns

`void`

***

### completeTransfer()

> **completeTransfer**(): `void`

#### Returns

`void`

***

### deflect()

#### Call Signature

> **deflect**(`target`): `void`

Deflect inbound calls (302) to a target: another `Agent` or a URI / number string.

##### Parameters

###### target

`Agent`

##### Returns

`void`

#### Call Signature

> **deflect**(`target`): `void`

##### Parameters

###### target

`string`

##### Returns

`void`

***

### dial()

#### Call Signature

> **dial**(`target`): `void`

Dial a target: another `Agent` (at its AOR) or a SIP URI / number string.

##### Parameters

###### target

`Agent`

##### Returns

`void`

#### Call Signature

> **dial**(`target`): `void`

##### Parameters

###### target

`string`

##### Returns

`void`

***

### dtmf()

> **dtmf**(`digits`, `gap?`): `void`

`dtmf(digits)` sends back-to-back; `dtmf(digits, gap)` inserts a pause
 (e.g. `"200ms"`) between digits.

#### Parameters

##### digits

`string`

##### gap?

`string`

#### Returns

`void`

***

### hangup()

> **hangup**(): `void`

#### Returns

`void`

***

### header()

> **header**(`name`): `string` \| `undefined`

First value of a received INVITE header (`a.header("X-Trace-Id")`).

#### Parameters

##### name

`string`

#### Returns

`string` \| `undefined`

***

### hold()

> **hold**(): `void`

#### Returns

`void`

***

### info()

> **info**(): [`AgentInfo`](AgentInfo.md)

A snapshot of the agent's observable state as an object.

#### Returns

[`AgentInfo`](AgentInfo.md)

***

### mute()

> **mute**(): `void`

#### Returns

`void`

***

### register()

> **register**(): `void`

#### Returns

`void`

***

### respondIncoming()

> **respondIncoming**(`code`, `reason`, `headers?`): `void`

Answer inbound INVITEs with a custom SIP response instead of accepting.
 `respondIncoming(486, "Busy Here")`, or with extra header lines:
 `respondIncoming(302, "Moved Temporarily", { Contact: "<sip:bob@example.com>" })`.

#### Parameters

##### code

`number`

##### reason

`string`

##### headers?

`Record`\<`string`, `string`\>

#### Returns

`void`

***

### resume()

> **resume**(): `void`

#### Returns

`void`

***

### sendAudio()

> **sendAudio**(`spec`): `void`

Set this agent's audio source on the active call (`a.sendAudio(tone(440))`).

#### Parameters

##### spec

[`AudioSpec`](AudioSpec.md)

#### Returns

`void`

***

### stopDeflect()

> **stopDeflect**(): `void`

#### Returns

`void`

***

### transfer()

#### Call Signature

> **transfer**(`target`): `void`

Blind-transfer the current call to a target: another `Agent` or a URI string.

##### Parameters

###### target

`Agent`

##### Returns

`void`

#### Call Signature

> **transfer**(`target`): `void`

##### Parameters

###### target

`string`

##### Returns

`void`

***

### verifyAudio()

> **verifyAudio**(`freq`, `within`): `Promise`\<`void`\>

Assert the agent receives a `freq` Hz tone within `within` (e.g. `"5s"`).
 Returns a Promise: the blocking detection window runs on the runtime's
 blocking pool, so `await Promise.all([a.verifyAudio(...), b.verifyAudio(...)])`
 listens on several agents concurrently instead of serially.

#### Parameters

##### freq

`number`

##### within

`string`

#### Returns

`Promise`\<`void`\>
