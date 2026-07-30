# Interface: HttpResponse

## Properties

### body

> `readonly` **body**: `string`

***

### status

> `readonly` **status**: `number`

## Methods

### expectStatus()

> **expectStatus**(`code`): `void`

#### Parameters

##### code

`number`

#### Returns

`void`

***

### header()

> **header**(`name`): `string` \| `undefined`

#### Parameters

##### name

`string`

#### Returns

`string` \| `undefined`

***

### json()

> **json**(`path?`): `any`

The JSON value at a dotted `path` (empty for the whole body), as a native
 JS value.

#### Parameters

##### path?

`string`

#### Returns

`any`
