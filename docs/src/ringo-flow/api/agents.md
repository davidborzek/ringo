# Agents

## Constructor

<a id="agent"></a>

### agent(name: string, config: map)

**Returns** [`Agent`](agents.md)

Connect a headless baresip agent and return a handle.

**Config options** — `agent(name, #{ … })`:

| Field | Type | Description |
| --- | --- | --- |
| `username` | string · required | SIP user (registration / auth) |
| `domain` | string · required | SIP domain / registrar |
| `password` | string | auth password |
| `display_name` | string | caller display name |
| `transport` | string | `udp` (default), `tcp` or `tls` |
| `auth_user` | string | auth user, if it differs from `username` |
| `outbound` | string | outbound proxy URI |
| `stun_server` | string | STUN server, e.g. `stun:host:port` |
| `media_enc` | string | media encryption, e.g. `srtp`, `zrtp`, `dtls_srtp` |
| `regint` | int | re-registration interval (seconds); `0` disables |
| `mwi` | bool | subscribe to message-waiting indication |
| `dtmf_mode` | string | `"info"` for reliable headless DTMF (SIP INFO) |
| `headers` | map | extra SIP headers on the INVITE, e.g. `#{ "X-Foo": "bar" }` |
| `deflect_to` | string | deflect inbound calls with a 302 to this URI/number (toggle at runtime with `deflect()`/`stop_deflect()`) |

**Example**

```rust
let a = agent("A", #{
    username: env("A_USER"),
    domain: env("SIP_DOMAIN"),
    password: env("A_PASS"),
});
```

## Methods

<a id="abort_transfer"></a>

### agent.abort_transfer()

**Receiver** [`Agent`](agents.md)

Abort the pending attended transfer.

<a id="accept"></a>

### agent.accept()

**Receiver** [`Agent`](agents.md)

Answer the agent's incoming call.

**Example**

```rust
await_until(|| assert(b.state).equals(State::Ringing), "15s");
b.accept();
```

<a id="attended_transfer"></a>

### agent.attended_transfer(target: Agent)

**Receiver** [`Agent`](agents.md) · **Takes** [`Agent`](agents.md)

Start an attended transfer: place a consultation call to another agent.
Complete it with `complete_transfer()` once that call is established.

**Example**

```rust
callee.attended_transfer(target);   // consult `target`
await_until(|| assert(target.state).equals(State::Established));
callee.complete_transfer();         // connect caller and target
```

<a id="attended_transfer"></a>

### agent.attended_transfer(target: string)

**Receiver** [`Agent`](agents.md)

Start an attended transfer to a literal URI or bare number.

<a id="complete_transfer"></a>

### agent.complete_transfer()

**Receiver** [`Agent`](agents.md)

Complete the pending attended transfer (REFER with Replaces).

<a id="deflect"></a>

### agent.deflect(target: Agent)

**Receiver** [`Agent`](agents.md) · **Takes** [`Agent`](agents.md)

Deflect inbound calls with a 302 Moved Temporarily to another agent's AOR
(a Diversion header names the deflecting agent). Arm it *before* the
caller dials; stays active until `stop_deflect()`.

**Example**

```rust
callee.deflect(target);     // future calls to `callee` go to `target`
caller.dial(callee);
await_until(|| assert(target.state).equals(State::Ringing), "15s");
```

<a id="deflect"></a>

### agent.deflect(target: string)

**Receiver** [`Agent`](agents.md)

Deflect inbound calls (302) to a literal URI or bare number/extension.

<a id="dial"></a>

### agent.dial(target: Agent)

**Receiver** [`Agent`](agents.md) · **Takes** [`Agent`](agents.md)

Dial another agent at its AOR.

**Example**

```rust
a.dial(b);                 // dial agent B at its AOR
a.dial("+49301234567");    // …or a number/URI in A's domain
await_until(|| assert(b.state).equals(State::Ringing), "15s");
```

<a id="dial"></a>

### agent.dial(target: string)

**Receiver** [`Agent`](agents.md)

Dial a literal SIP URI, or a bare number/extension in the agent's own domain.

<a id="dtmf"></a>

### agent.dtmf(digits: string)

**Receiver** [`Agent`](agents.md)

Send DTMF tones (characters `0-9`, `*`, `#`, `A-D`) back-to-back.

**Example**

```rust
a.dtmf("123#");
```

<a id="dtmf"></a>

### agent.dtmf(digits: string, gap: string)

**Receiver** [`Agent`](agents.md)

Send DTMF tones with a pause between digits.

**Example**

```rust
a.dtmf("123#", "200ms");
```

<a id="hangup"></a>

### agent.hangup()

**Receiver** [`Agent`](agents.md)

Hang up the agent's active call.

**Example**

```rust
a.hangup();
await_until(|| assert(a.state).equals(State::Idle), "10s");
```

<a id="header"></a>

### agent.header(name: string)

**Receiver** [`Agent`](agents.md) · **Returns** `string?`

Value of a header on a received INVITE (string), or `()` if absent.

<a id="headers"></a>

### agent.headers()

**Receiver** [`Agent`](agents.md) · **Returns** `map`

All received INVITE headers as a map (name → value); duplicates collapse,
use `header(name)` for a specific one.

<a id="hold"></a>

### agent.hold()

**Receiver** [`Agent`](agents.md)

Put the active call on hold.

<a id="info"></a>

### agent.info()

**Receiver** [`Agent`](agents.md) · **Returns** `map`

A map of the agent's current state: name, aor, registered, state,
reason, status_code, calls. Handy to `print(...)` or assert on.

<a id="mute"></a>

### agent.mute()

**Receiver** [`Agent`](agents.md)

Toggle mute on the active call.

**Example**

```rust
a.mute(); // mute; call again to unmute
```

<a id="register"></a>

### agent.register()

**Receiver** [`Agent`](agents.md)

(Re-)register the agent's account.

**Example**

```rust
a.register();
await_until(|| assert(a.registered).is_true(), "10s");
```

<a id="respond_incoming"></a>

### agent.respond_incoming(status: int, reason: string)

**Receiver** [`Agent`](agents.md)

Answer inbound INVITEs with a custom SIP response (status + reason)
instead of accepting — e.g. `callee.respond_incoming(486, "Busy Here")`.
Arm before the caller dials; clear with `stop_deflect()`.

<a id="respond_incoming"></a>

### agent.respond_incoming(status: int, reason: string, headers: map)

**Receiver** [`Agent`](agents.md)

Custom response with extra headers, e.g.
`callee.respond_incoming(302, "Moved Temporarily", #{ "Contact": "<sip:bob@example.com>" })`.
Header values must not contain CR/LF.

<a id="resume"></a>

### agent.resume()

**Receiver** [`Agent`](agents.md)

Resume a held call.

<a id="send_audio"></a>

### agent.send_audio(source: AudioSpec)

**Receiver** [`Agent`](agents.md) · **Takes** [`AudioSpec`](audiospec.md)

Switch the agent's active-call audio source: `tone(Hz)`, `file(path)` or `silent()`.

**Example**

```rust
a.send_audio(tone(440));         // play a 440 Hz tone
a.send_audio(file("prompt.wav"));
```

<a id="stop_deflect"></a>

### agent.stop_deflect()

**Receiver** [`Agent`](agents.md)

Stop deflecting / clear any armed response — inbound calls are accepted again.

<a id="to_json"></a>

### agent.to_json()

**Receiver** [`Agent`](agents.md) · **Returns** `string`

The agent's current state as a JSON string (for `log(...)`/debugging).

<a id="transfer"></a>

### agent.transfer(target: Agent)

**Receiver** [`Agent`](agents.md) · **Takes** [`Agent`](agents.md)

Blind-transfer (REFER) the active call to another agent's AOR.

**Example**

```rust
callee.transfer(target); // hand the caller off to `target`
```

<a id="transfer"></a>

### agent.transfer(target: string)

**Receiver** [`Agent`](agents.md)

Blind-transfer (REFER) the active call to a literal URI or bare number.

<a id="verify_audio"></a>

### agent.verify_audio(freq: int, within: string)

**Receiver** [`Agent`](agents.md)

Assert the agent is receiving a tone at `freq` Hz within the window (Goertzel).

**Example**

```rust
a.send_audio(tone(440));
b.verify_audio(440, "5s"); // b hears A's 440 Hz tone
```

<a id="verify_audio_connection"></a>

### agent.verify_audio_connection(b: Agent)

**Receiver** [`Agent`](agents.md) · **Takes** [`Agent`](agents.md)

Assert two-way audio between two agents (a→b then b→a) at 1000 Hz.

**Example**

```rust
caller.verify_audio_connection(callee);
```

## Fields

<a id="peer"></a>

### agent.peer

**Receiver** [`Agent`](agents.md) · **Returns** [`Peer`](peer.md)

The current call's remote party (the caller for an incoming call); read
`peer.uri` / `peer.number` / `peer.name` (each `()` if there's no call).

<a id="quality"></a>

### agent.quality

**Receiver** [`Agent`](agents.md) · **Returns** `CallQuality`

RTP media quality of the active call (or the last call's snapshot); read
`quality.mos` / `.rtt` / `.jitter` / `.packet_loss` (each `()` until the
first RTCP report, ~5s into a call).

**Example**

```rust
await_until(|| assert(caller.quality.mos).is_present(), "10s");
assert(caller.quality.mos).at_least(4.0);
```

<a id="reason"></a>

### agent.reason

**Receiver** [`Agent`](agents.md) · **Returns** `string?`

The last closed call's reason (string), or `()` if none yet.

<a id="registered"></a>

### agent.registered

**Receiver** [`Agent`](agents.md) · **Returns** `bool`

Whether the agent's account is currently registered.

<a id="state"></a>

### agent.state

**Receiver** [`Agent`](agents.md) · **Returns** [`CallState`](call-state.md)

The agent's current call phase: `Idle`, `Ringing` or `Established`.

<a id="status_code"></a>

### agent.status_code

**Receiver** [`Agent`](agents.md) · **Returns** `int?`

SIP status code from the last closed call's reason (int, e.g. `603`),
or `()` if the reason isn't a SIP response (local hangup, reset, …).

