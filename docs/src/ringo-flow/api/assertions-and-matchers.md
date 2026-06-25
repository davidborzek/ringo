# Assertions and matchers

## Constructor

<a id="assert"></a>

### assert(actual)

**Returns** [`Assertion`](assertions-and-matchers.md)

Begin a fluent assertion on a value: `assert(x).equals(y)`, `.is_true()`,
`.greater_than(n)`, etc. Matchers chain (`.at_least(200).at_most(299)`)
and error (with a value-based message) on a mismatch. Asserting on a
getter auto-labels the log line (`assert(caller.state)` → `Caller state`,
`assert(res.status)` → `HTTP status`); `.describe(…)` overrides.

## Methods

<a id="at_least"></a>

### assertion.at_least(n: int)

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the (numeric) value is >= `n`.

<a id="at_most"></a>

### assertion.at_most(n: int)

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the (numeric) value is <= `n`.

<a id="contains"></a>

### assertion.contains(needle: string)

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the (string) value contains `needle`.

**Example**

```rust
assert(a.header("User-Agent")).contains("baresip");
```

<a id="describe"></a>

### assertion.describe(label: string)

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Label this assertion so the log line names it: `assert(caller.registered)
.describe("caller registered").is_true()` → `caller registered: ✓ expect …`.

<a id="equals"></a>

### assertion.equals(expected)

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the value equals `expected` (`is` is a reserved word in Rhai).

**Example**

```rust
assert(a.state).equals(State::Established);
```

<a id="greater_than"></a>

### assertion.greater_than(n: int)

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the (numeric) value is > `n`.

<a id="is_absent"></a>

### assertion.is_absent()

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the value is absent (`()`).

<a id="is_empty"></a>

### assertion.is_empty()

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the string/array/map value is empty.

<a id="is_false"></a>

### assertion.is_false()

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the value is `false`.

<a id="is_not_empty"></a>

### assertion.is_not_empty()

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the string/array/map value is not empty.

<a id="is_present"></a>

### assertion.is_present()

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the value is present (not `()`), e.g. a received header.

<a id="is_true"></a>

### assertion.is_true()

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the value is `true`.

**Example**

```rust
assert(a.registered).is_true();
```

<a id="less_than"></a>

### assertion.less_than(n: int)

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the (numeric) value is < `n`.

<a id="matches"></a>

### assertion.matches(pattern: string)

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the (string) value matches the regex `pattern`.

<a id="not_equals"></a>

### assertion.not_equals(expected)

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** [`Assertion`](assertions-and-matchers.md)

Assert the value does not equal `expected`.

<a id="value"></a>

### assertion.value()

**Receiver** [`Assertion`](assertions-and-matchers.md) · **Returns** `any`

The value under assertion, so a verified value can be bound:
`let id = await_until(|| assert(callee.header("X-Id")).is_present().value());`.

