use ringo_speech::TtsConfig;

fn main() -> anyhow::Result<()> {
    let tts = ringo_speech::load_tts(&TtsConfig {
        model_dir: std::path::PathBuf::from(
            std::env::var("RINGO_TTS_MODEL").expect("RINGO_TTS_MODEL=<voice dir>"),
        ),
        speed: 1.0,
    })?;
    let start = std::time::Instant::now();
    let out = tts.synth("Hallo, hier spricht ringo. Dies ist ein Test.")?;
    eprintln!(
        "synth: {} samples @ {} Hz = {:?} in {:?}",
        out.samples.len(),
        out.sample_rate,
        out.duration(),
        start.elapsed()
    );
    let rms = (out
        .samples
        .iter()
        .map(|&s| (s as f64) * (s as f64))
        .sum::<f64>()
        / out.samples.len() as f64)
        .sqrt();
    eprintln!("rms: {rms:.1} (silence would be 0)");
    let out_path =
        std::env::var("RINGO_TTS_OUT").unwrap_or_else(|_| "/tmp/ringo-speech-test.wav".into());
    ringo_speech::write_mono_wav(
        std::path::Path::new(&out_path),
        &out.samples,
        out.sample_rate,
    )?;
    eprintln!("wav: {out_path}");
    Ok(())
}
