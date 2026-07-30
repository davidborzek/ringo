# Function: setup()

> **setup**(`body`): `void`

Register a `setup(fn)` body, run before every scenario. Its return value becomes
 the per-scenario context passed to the body (and to `teardown`).

## Parameters

### body

() => `any`

## Returns

`void`
