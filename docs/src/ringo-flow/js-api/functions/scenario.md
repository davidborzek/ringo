# Function: scenario()

## Call Signature

> **scenario**(`name`, `body`): `void`

Register a `scenario(...)`: with a third arg, `a` is the options object and
 `b` the body; otherwise `a` is the body. Persists the body and records
 tags/skip/only from the options.

### Parameters

#### name

`string`

#### body

[`ScenarioBody`](../type-aliases/ScenarioBody.md)

### Returns

`void`

## Call Signature

> **scenario**(`name`, `opts`, `body`): `void`

Register a `scenario(...)`: with a third arg, `a` is the options object and
 `b` the body; otherwise `a` is the body. Persists the body and records
 tags/skip/only from the options.

### Parameters

#### name

`string`

#### opts

[`ScenarioOptions`](../interfaces/ScenarioOptions.md)

#### body

[`ScenarioBody`](../type-aliases/ScenarioBody.md)

### Returns

`void`
