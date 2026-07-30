# Interface: Assertion\<T\>

## Type Parameters

### T

`T`

## Properties

### not

> `readonly` **not**: `Assertion`\<`T`\>

Negate the next matcher (Jest-style): `expect(x).not.toBe(2)`. Applies only to
 the matcher immediately after — the handle it returns is positive again.

## Methods

### as()

> **as**(`label`): `Assertion`\<`T`\>

Label this assertion (`.as("caller registered")`) — chainable, Jest has no
 equivalent so the name avoids colliding with a test-grouping `describe`.

#### Parameters

##### label

`string`

#### Returns

`Assertion`\<`T`\>

***

### toBe()

> **toBe**(`expected`): `Assertion`\<`T`\>

#### Parameters

##### expected

`T`

#### Returns

`Assertion`\<`T`\>

***

### toBeDefined()

> **toBeDefined**(): `Assertion`\<`T`\>

#### Returns

`Assertion`\<`T`\>

***

### toBeEmpty()

> **toBeEmpty**(): `Assertion`\<`T`\>

#### Returns

`Assertion`\<`T`\>

***

### toBeFalsy()

> **toBeFalsy**(): `Assertion`\<`T`\>

#### Returns

`Assertion`\<`T`\>

***

### toBeGreaterThan()

> **toBeGreaterThan**(`n`): `Assertion`\<`T`\>

#### Parameters

##### n

`number`

#### Returns

`Assertion`\<`T`\>

***

### toBeGreaterThanOrEqual()

> **toBeGreaterThanOrEqual**(`n`): `Assertion`\<`T`\>

#### Parameters

##### n

`number`

#### Returns

`Assertion`\<`T`\>

***

### toBeLessThan()

> **toBeLessThan**(`n`): `Assertion`\<`T`\>

#### Parameters

##### n

`number`

#### Returns

`Assertion`\<`T`\>

***

### toBeLessThanOrEqual()

> **toBeLessThanOrEqual**(`n`): `Assertion`\<`T`\>

#### Parameters

##### n

`number`

#### Returns

`Assertion`\<`T`\>

***

### toBeTruthy()

> **toBeTruthy**(): `Assertion`\<`T`\>

#### Returns

`Assertion`\<`T`\>

***

### toBeUndefined()

> **toBeUndefined**(): `Assertion`\<`T`\>

#### Returns

`Assertion`\<`T`\>

***

### toContain()

> **toContain**(`needle`): `Assertion`\<`T`\>

#### Parameters

##### needle

`string`

#### Returns

`Assertion`\<`T`\>

***

### toMatch()

> **toMatch**(`pattern`): `Assertion`\<`T`\>

#### Parameters

##### pattern

`string`

#### Returns

`Assertion`\<`T`\>

***

### value()

> **value**(): `T`

The value under assertion, so a verified value can be bound, e.g.
 `const id = await until(() => expect(callee.header("X-Id")).toBeDefined().value())`.

#### Returns

`T`
