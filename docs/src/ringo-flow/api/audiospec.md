# AudioSpec

<a id="file"></a>

## file(path: string)

**Returns** [`AudioSpec`](audiospec.md)

A WAV-file audio source, for `send_audio`.

<a id="silent"></a>

## silent()

**Returns** [`AudioSpec`](audiospec.md)

A silent audio source (stop sending), for `send_audio`.

<a id="tone"></a>

## tone(freq: int)

**Returns** [`AudioSpec`](audiospec.md)

A sine-tone audio source at the given frequency (Hz), for `send_audio`.

**Example**

```rust
a.send_audio(tone(440));
```

