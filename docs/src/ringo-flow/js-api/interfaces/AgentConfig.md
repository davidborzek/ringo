# Interface: AgentConfig

## Properties

### auth\_user?

> `optional` **auth\_user?**: `string`

auth user, if it differs from `username`.

***

### deflect\_to?

> `optional` **deflect\_to?**: `string`

deflect inbound calls with a 302 to this URI/number.

***

### display\_name?

> `optional` **display\_name?**: `string`

caller display name.

***

### domain

> **domain**: `string`

SIP domain / registrar. Required.

***

### dtmf\_mode?

> `optional` **dtmf\_mode?**: `string`

`"info"` for reliable headless DTMF (SIP INFO).

***

### headers?

> `optional` **headers?**: `Record`\<`string`, `string` \| `string`[]\>

(`{ "X-Foo": ["a", "b"] }`) sends the header repeated, once per element.

***

### media\_enc?

> `optional` **media\_enc?**: `string`

media encryption, e.g. `srtp`, `zrtp`, `dtls_srtp`.

***

### metadata?

> `optional` **metadata?**: `Record`\<`string`, `unknown`\>

(e.g. `{ role: "caller" }`); not used for SIP.

***

### mwi?

> `optional` **mwi?**: `boolean`

subscribe to message-waiting indication.

***

### outbound?

> `optional` **outbound?**: `string`

outbound proxy URI.

***

### password?

> `optional` **password?**: `string`

Auth password.

***

### regint?

> `optional` **regint?**: `number`

re-registration interval (seconds); `0` disables.

***

### stun\_server?

> `optional` **stun\_server?**: `string`

STUN server, e.g. `stun:host:port`.

***

### transport?

> `optional` **transport?**: `string`

`udp` (default), `tcp` or `tls`.

***

### username

> **username**: `string`

SIP user (registration / auth). Required.
