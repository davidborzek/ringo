# Audio testing

ringo-flow runs baresip with virtual audio, so it can both **play** audio into a
call and **check** what the other side receives — headless, no devices, CI-safe.

## Send audio

[`agent.send_audio(source)`](api/agents.md#send_audio) switches the agent's
active-call audio source:

```rust
a.send_audio(tone(440));          // a 440 Hz sine tone
a.send_audio(file("prompt.wav")); // a WAV file
a.send_audio(silent());           // stop sending
```

[`tone`](api/audiospec.md#tone), [`file`](api/audiospec.md#file) and
[`silent`](api/audiospec.md#silent) build an [`AudioSpec`](api/audiospec.md).

## Verify what's received

[`agent.verify_audio(freq, within)`](api/agents.md#verify_audio) asserts the agent
is receiving a tone at `freq` Hz within the time window (detected with a Goertzel
filter):

```rust
a.send_audio(tone(440));
b.verify_audio(440, "5s"); // B hears A's tone within 5s
```

For a quick two-way check, [`verify_audio_connection(a, b)`](api/agents.md#verify_audio_connection)
sends a tone each way and asserts both arrive:

```rust
verify_audio_connection(a, b);
```

## Debugging

Run with `--save-audio` to write each agent's sent/received WAVs to the working
directory, so you can listen to what actually flowed.

See the [Agents → Methods](api/agents.md) reference for the exact signatures.
