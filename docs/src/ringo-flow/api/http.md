# HTTP

## Constructor

<a id="http"></a>

### http(method: string, url: string)

**Returns** [`HttpResponse`](http.md)

Make an HTTP request and return the response.

**Example**

```rust
let res = http("GET", env("API_URL") + "/calls");
res.expect_status(200);
```

<a id="http"></a>

### http(method: string, url: string, options: map)

**Returns** [`HttpResponse`](http.md)

Make an HTTP request with options and return the response.

**Options** — `http(method, url, #{ … })`:

| Field | Type | Description |
| --- | --- | --- |
| `headers` | map | request headers, e.g. `#{ "Content-Type": "application/json" }` |
| `body` | string or map | request body; a map is encoded to JSON |

**Example**

```rust
let res = http("POST", env("API_URL") + "/calls", #{
    headers: #{ "Content-Type": "application/json" },
    body: #{ to: "+49301234567" },
});
```

## Methods

<a id="expect_status"></a>

### resp.expect_status(code: int)

**Receiver** [`HttpResponse`](http.md)

Assert and report the status; errors on mismatch.

<a id="header"></a>

### resp.header(name: string)

**Receiver** [`HttpResponse`](http.md) · **Returns** `string?`

A response header value (string), or `()` if absent.

<a id="json"></a>

### resp.json()

**Receiver** [`HttpResponse`](http.md)

The whole JSON body as a native value (object→map, array, …).

<a id="json"></a>

### resp.json(path: string)

**Receiver** [`HttpResponse`](http.md)

The value at a dotted JSON path (e.g. `"data.id"`), typed: object→map,
array, number, bool, `null`→`()`. Errors if the path is missing.

**Example**

```rust
assert(res.json("data.id")).equals(42);
```

## Fields

<a id="body"></a>

### resp.body

**Receiver** [`HttpResponse`](http.md) · **Returns** `string`

The HTTP response body as a string.

<a id="status"></a>

### resp.status

**Receiver** [`HttpResponse`](http.md) · **Returns** `int`

The HTTP response status code.

