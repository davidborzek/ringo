# Class: MockServer

## Constructors

### Constructor

> **new MockServer**(`opts?`): `MockServer`

#### Parameters

##### opts?

###### port?

`number`

#### Returns

`MockServer`

## Properties

### port

> `readonly` **port**: `number`

***

### url

> `readonly` **url**: `string`

The server's base URL (`http://127.0.0.1:<port>`), to point the SUT at.

## Methods

### lastRequest()

> **lastRequest**(`path`): [`MockRequestInfo`](../interfaces/MockRequestInfo.md) \| `undefined`

The most recent request on `path` (string or `regex(...)`) as
 `{ method, path, query, headers, body }`, or `undefined`.

#### Parameters

##### path

`string` \| [`PathMatch`](../interfaces/PathMatch.md)

#### Returns

[`MockRequestInfo`](../interfaces/MockRequestInfo.md) \| `undefined`

***

### requestCount()

> **requestCount**(`path`): `number`

How many requests arrived on `path` (string or `regex(...)`, any method) —
 poll via `until`.

#### Parameters

##### path

`string` \| [`PathMatch`](../interfaces/PathMatch.md)

#### Returns

`number`

***

### requests()

> **requests**(`path`): [`MockRequestInfo`](../interfaces/MockRequestInfo.md)[]

All requests on `path` (string or `regex(...)`), in arrival order.

#### Parameters

##### path

`string` \| [`PathMatch`](../interfaces/PathMatch.md)

#### Returns

[`MockRequestInfo`](../interfaces/MockRequestInfo.md)[]

***

### respond()

#### Call Signature

> **respond**(`method`, `path`, `response`): `void`

Register a route: a static response object, or a per-request closure (runs on
 the scenario thread, pumped from `until`). `path` is a string or
 `regex(...)`; a leading method arg is optional.

##### Parameters

###### method

`string`

###### path

`string` \| [`PathMatch`](../interfaces/PathMatch.md)

###### response

[`MockResponder`](../type-aliases/MockResponder.md)

##### Returns

`void`

#### Call Signature

> **respond**(`path`, `response`): `void`

Register a route: a static response object, or a per-request closure (runs on
 the scenario thread, pumped from `until`). `path` is a string or
 `regex(...)`; a leading method arg is optional.

##### Parameters

###### path

`string` \| [`PathMatch`](../interfaces/PathMatch.md)

###### response

[`MockResponder`](../type-aliases/MockResponder.md)

##### Returns

`void`

***

### stop()

> **stop**(): `void`

Stop the server early (it otherwise stops at scenario teardown).

#### Returns

`void`
