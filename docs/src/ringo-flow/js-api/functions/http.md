# Function: http()

> **http**(`method`, `url`, `opts?`): `Promise`\<[`HttpResponse`](../interfaces/HttpResponse.md)\>

Performs the request off-thread and resolves with the response; `await` it.
 `await Promise.all([http(...), http(...)])` fires several requests concurrently.

## Parameters

### method

`string`

### url

`string`

### opts?

[`HttpOptions`](../interfaces/HttpOptions.md)

## Returns

`Promise`\<[`HttpResponse`](../interfaces/HttpResponse.md)\>
