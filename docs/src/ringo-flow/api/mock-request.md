# Mock request

## Methods

<a id="header"></a>

### req.header(name: string)

**Receiver** [`MockRequest`](mock-request.md) · **Returns** `string?`

A request header value (case-insensitive), or `()` if absent.

<a id="json"></a>

### req.json(path: string)

**Receiver** [`MockRequest`](mock-request.md)

The value at a dotted JSON path in the body (object→map, array, number,
bool, `null`→`()`). Errors if the path is missing.

**Example**

```rust
assert(req.json("call.from")).equals("+49301234567");
```

<a id="query"></a>

### req.query(name: string)

**Receiver** [`MockRequest`](mock-request.md) · **Returns** `string?`

A query-string parameter value, or `()` if absent.

## Fields

<a id="body"></a>

### req.body

**Receiver** [`MockRequest`](mock-request.md) · **Returns** `string`

The raw request body.

<a id="method"></a>

### req.method

**Receiver** [`MockRequest`](mock-request.md) · **Returns** `string`

The request method (upper-case).

<a id="path"></a>

### req.path

**Receiver** [`MockRequest`](mock-request.md) · **Returns** `string`

The request path.

