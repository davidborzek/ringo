//! The `AudioSpec` JS class and the audio-source globals (`tone`/`file`/`silence`)
//! plus the async `verifyAudioConnection(...)` global.

use super::super::tsgen::declare;
use super::agent::Agent;
use super::core::HostState;
use crate::engine::audio::AudioSpec as EngAudioSpec;
use rquickjs::class::Trace;
use rquickjs::{Class, Ctx, JsLifetime};


/// An audio source for `sendAudio` — built via the `tone`/`file`/`silence`
/// globals. Opaque handle over the engine's `AudioSpec`.
#[derive(Trace, JsLifetime, Clone)]
#[rquickjs::class(rename = "AudioSpec")]
pub struct AudioSpec {
    #[qjs(skip_trace)]
    pub inner: EngAudioSpec,
}

// Opaque handle (no JS-visible members) — declared verbatim, the branded `never`
// field makes it nominal so a plain `{}` can't be passed where an `AudioSpec` is wanted.
declare!("interface AudioSpec { readonly __audioSpec?: never; }");

/// A constant-tone audio source for `sendAudio`.
#[ringo_flow_macros::ts_global(name = "tone")]
pub(in crate::script::js) fn audio_tone<'js>(cx: Ctx<'js>, freq: i64) -> rquickjs::Result<Class<'js, AudioSpec>> {
    Class::instance(
        cx,
        AudioSpec {
            inner: EngAudioSpec::Tone(freq.max(0) as u32),
        },
    )
}
/// A WAV-file audio source for `sendAudio`.
#[ringo_flow_macros::ts_global(name = "file")]
pub(in crate::script::js) fn audio_file<'js>(cx: Ctx<'js>, path: String) -> rquickjs::Result<Class<'js, AudioSpec>> {
    Class::instance(cx, AudioSpec { inner: EngAudioSpec::File(path) })
}
/// A silent audio source for `sendAudio`.
#[ringo_flow_macros::ts_global(name = "silence")]
pub(in crate::script::js) fn audio_silence(cx: Ctx<'_>) -> rquickjs::Result<Class<'_, AudioSpec>> {
    Class::instance(cx, AudioSpec { inner: EngAudioSpec::Silent })
}

/// Assert two-way audio between two agents (a→b then b→a); resolves on success.
/// Blocking detection runs on the runtime's blocking pool so the JS thread is free.
#[ringo_flow_macros::ts_global(name = "verifyAudioConnection")]
pub(in crate::script::js) async fn verify_audio_connection_async<'js>(
    cx: Ctx<'js>,
    a: Class<'js, Agent>,
    b: Class<'js, Agent>,
) -> rquickjs::Result<()> {
    let eng = cx
        .userdata::<HostState>()
        .expect("host state stored at install")
        .eng
        .clone();
    let an = a.borrow().name.clone();
    let bn = b.borrow().name.clone();
    let handle = eng.rt.clone();
    match handle
        .spawn_blocking(move || crate::engine::audio::verify_audio_connection(&eng, &an, &bn))
        .await
    {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(super::super::convert::throw(&cx, &e)),
        Err(e) => Err(super::super::convert::throw(&cx, &format!("verifyAudioConnection task failed: {e}"))),
    }
}
