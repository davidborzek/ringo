# Audio testing

ringo-flow runs baresip with virtual audio, so it can both **play** audio into a
call and **check** what the other side receives — headless, no devices, CI-safe.

## Send audio

[`agent.sendAudio(source)`](js-api/classes/Agent.md#sendaudio) switches the
agent's active-call audio source:

```js
a.sendAudio(tone(440));          // a 440 Hz sine tone
a.sendAudio(file("prompt.wav")); // a WAV file
a.sendAudio(silence());          // stop sending
```

[`tone`](js-api/functions/tone.md), [`file`](js-api/functions/file.md) and
[`silence`](js-api/functions/silence.md) build an
[`AudioSpec`](js-api/interfaces/AudioSpec.md).

## Verify what's received

[`agent.verifyAudio(freq, within)`](js-api/classes/Agent.md#verifyaudio) asserts
the agent is receiving a tone at `freq` Hz within the time window (detected with a
Goertzel filter). It returns a Promise, so `await` it:

```js
a.sendAudio(tone(440));
await b.verifyAudio(440, "5s"); // B hears A's tone within 5s
```

The blocking detection window runs off the scenario thread, so several agents can
listen at once instead of one after another:

```js
a.sendAudio(tone(440));
b.sendAudio(tone(480));
await Promise.all([b.verifyAudio(440, "5s"), a.verifyAudio(480, "5s")]);
```

For a quick two-way check,
[`verifyAudioConnection(a, b)`](js-api/functions/verifyAudioConnection.md) sends a
tone each way and asserts both arrive:

```js
await verifyAudioConnection(a, b);
```

## Debugging

Run with `--save-audio` to write each agent's sent/received WAVs to the working
directory, so you can listen to what actually flowed.

See the [Agent](js-api/classes/Agent.md) reference for the exact signatures.
