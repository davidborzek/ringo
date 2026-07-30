# Interface: MockResponseSpec

## Properties

### body?

> `optional` **body?**: `string`

Response body (a string; use `jsonResponse`/`textResponse` for shorthands).

***

### contentType?

> `optional` **contentType?**: `string`

`Content-Type` header to set.

***

### headers?

> `optional` **headers?**: `Record`\<`string`, `string`\>

Extra response headers.

***

### status?

> `optional` **status?**: `number`

HTTP status code to return (default `200`).
