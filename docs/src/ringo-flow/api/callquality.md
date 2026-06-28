# CallQuality

<a id="jitter"></a>

## quality.jitter

**Receiver** `CallQuality` · **Returns** `float?`

Receive-side jitter in milliseconds, or `()` if not available yet.

<a id="mos"></a>

## quality.mos

**Receiver** `CallQuality` · **Returns** `float?`

Estimated MOS (1.0–4.5), or `()` until the first RTCP report.

<a id="packet_loss"></a>

## quality.packet_loss

**Receiver** `CallQuality` · **Returns** `float?`

Receive-side packet loss in percent, or `()` if not available yet.

<a id="rtt"></a>

## quality.rtt

**Receiver** `CallQuality` · **Returns** `float?`

Round-trip time in milliseconds, or `()` if not available yet.

