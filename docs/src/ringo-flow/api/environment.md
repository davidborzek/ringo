# Environment

<a id="env"></a>

## env(name: string)

**Returns** `string`

Read a variable: first from `--env-file`/`<scenario>.env`/`load_env`, then
the process environment. Errors if unset. Use it for per-env credentials.

**Example**

```rust
let dom = env("SIP_DOMAIN");
let a = agent("A", #{ username: env("A_USER"), domain: dom, password: env("A_PASS") });
```

<a id="load_env"></a>

## load_env(path: string)

Load a dotenv file (`KEY=VALUE` lines) into `env(...)` for this scenario,
resolved relative to the scenario file. Later loads override earlier keys.

