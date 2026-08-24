//! Telephony tone synthesis.
//!
//! Call-progress tones are not recordings — they are specifications (ITU-T
//! E.180 and the national supplements): a few sine frequencies in a fixed
//! on/off cadence. Generating them beats shipping WAV files on every count:
//! nothing to license, any sample rate we like instead of 8 kHz mu-law, an
//! exact cadence, and a seamless loop.
//!
//! A tone is written the way Asterisk's `indications.conf` writes it, so any of
//! its 50 country zones can be pasted in verbatim:
//!
//! ```text
//! 425/1000,0/4000        Germany, ringback: 425 Hz for 1 s, then 4 s of silence
//! 440+480/2000,0/4000    North America, ringback: two mixed frequencies
//! ```
//!
//! Elements are separated by commas; each is `freq[+freq…]/milliseconds`, and a
//! frequency of `0` is silence. Asterisk's `*` (modulation) and `!` (play once)
//! are not supported — say so in the error rather than mis-playing them.

use std::fmt;

/// One stretch of a tone: which frequencies sound, and for how long.
/// An empty `freqs` is silence — that is how a cadence's gap is written.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub freqs: Vec<f64>,
    pub ms: u32,
}

/// A complete tone: the segments of exactly one cadence period.
#[derive(Debug, Clone, PartialEq)]
pub struct Tone {
    pub segments: Vec<Segment>,
}

// ─── The built-in tones ──────────────────────────────────────────────────────
//
// Written as specs rather than built as structs, so the defaults go through the
// very same parser a user's config does — one code path, exercised on every
// start.

/// Germany, ringback (Freiton). Shared with most of Europe and, per ITU-T,
/// much of the world. Asterisk zone `[de]`, `ring`.
pub const DE_RINGBACK: &str = "425/1000,0/4000";

/// Germany, busy (Besetztton) — the called party is on the phone.
/// Asterisk zone `[de]`, `busy`.
pub const DE_BUSY: &str = "425/480,0/480";

/// Germany, congestion (Gassenbesetztton) — the network could not put the call
/// through. Same frequency as busy at twice the rate, which is exactly the
/// distinction between a SIP 486 and any other failure.
/// Asterisk zone `[de]`, `congestion`.
pub const DE_CONGESTION: &str = "425/240,0/240";

// ─── Chimes ──────────────────────────────────────────────────────────────────
//
// Ring and message are not signals, they are sounds — no specification says
// what an incoming call should sound like. But "a short melodic motif of struck
// notes" is a description, not a recording: a fundamental with its partials,
// decaying the way something physically hit decays. That is a few lines of
// arithmetic, and it leaves ringo with no third-party audio at all.

/// A partial of a struck note: its frequency as a multiple of the fundamental,
/// and its amplitude relative to it.
pub struct Partial {
    pub ratio: f64,
    pub amp: f64,
}

const fn partial(ratio: f64, amp: f64) -> Partial {
    Partial { ratio, amp }
}

/// How a struck note is coloured and how long it rings.
pub struct Timbre {
    pub name: &'static str,
    pub partials: &'static [Partial],
    /// Time to fade to inaudibility. Short reads as wooden, long as metallic.
    pub decay_ms: u32,
}

/// Nearly a pure sine, with a touch of the octave — the quietest of the
/// timbres we tried, and the one that stays bearable on the tenth repetition.
/// Adding another is six lines: partials as multiples of the fundamental, plus
/// a decay. A marimba is 1/4/10 and 600 ms, struck glass 1/2/2.4/3/4.5 and
/// 1100 ms.
pub const SOFT: Timbre = Timbre {
    name: "soft",
    partials: &[partial(1.0, 1.0), partial(2.0, 0.14)],
    decay_ms: 500,
};

/// A short motif of struck notes, plus the silence that follows it — so a
/// looping alert carries its own cadence and needs no gap bolted on.
pub struct Chime {
    /// Note frequencies in Hz, played in order.
    pub freqs: &'static [f64],
    /// Time between note onsets. Shorter than the decay, so notes ring into
    /// one another instead of being cut off — that overlap is most of what
    /// makes this sound designed rather than beeped.
    pub spacing_ms: u32,
    pub timbre: &'static Timbre,
    /// One full period, motif plus trailing silence.
    pub period_ms: u32,
}

/// Incoming call: an ascending G–C–E triad, repeating every 2.5 s. Modern
/// ringtones are a short motif in a cadence, not a continuous tone.
pub const RING: Chime = Chime {
    freqs: &[783.99, 1046.50, 1318.51],
    spacing_ms: 170,
    timbre: &SOFT,
    period_ms: 2500,
};

/// New voicemail: two descending notes, played once. Deliberately smaller than
/// the ring — it reports something, it does not ask for you.
pub const MESSAGE: Chime = Chime {
    freqs: &[1046.50, 783.99],
    spacing_ms: 200,
    timbre: &SOFT,
    period_ms: 1200,
};

/// Render one period of `chime` as mono S16 samples at `srate`.
pub fn render_chime(chime: &Chime, srate: u32) -> Vec<i16> {
    let n = (srate as u64 * chime.period_ms as u64 / 1000) as usize;
    let mut buf = vec![0.0f64; n];
    // Amplitude falls to -60 dB after decay_ms.
    let tau = chime.timbre.decay_ms as f64 / 1000.0 / 6.9;
    let attack = (srate as f64 * 0.003) as usize; // 3 ms, enough to kill the click
    for (i, &freq) in chime.freqs.iter().enumerate() {
        let onset = (srate as u64 * (i as u64 * chime.spacing_ms as u64) / 1000) as usize;
        for (age, slot) in buf[onset..].iter_mut().enumerate() {
            let t = age as f64 / srate as f64;
            let decay = (-t / tau).exp();
            if decay < 0.0005 {
                break;
            }
            let rise = if age < attack {
                age as f64 / attack as f64
            } else {
                1.0
            };
            let mut v = 0.0;
            for p in chime.timbre.partials {
                v += p.amp * (std::f64::consts::TAU * freq * p.ratio * t).sin();
            }
            *slot += v * decay * rise;
        }
    }
    // Notes overlap, so the peak is whatever it turns out to be — normalize
    // rather than trying to budget amplitudes per note.
    let peak = buf.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let gain = if peak > 0.0 { LEVEL / peak } else { 0.0 };
    buf.iter()
        .map(|v| {
            (v * gain * i16::MAX as f64)
                .round()
                .clamp(-32768.0, 32767.0) as i16
        })
        .collect()
}

// ─── Parsing ─────────────────────────────────────────────────────────────────

/// Guard rails. Generous enough for any real cadence, tight enough that a
/// mistyped config cannot allocate its way through memory.
const MAX_SEGMENTS: usize = 32;
const MAX_FREQS: usize = 4;
const MAX_TOTAL_MS: u32 = 60_000;
const FREQ_RANGE: std::ops::RangeInclusive<f64> = 20.0..=20_000.0;

impl std::str::FromStr for Tone {
    type Err = ParseError;

    fn from_str(spec: &str) -> Result<Self, ParseError> {
        let spec = spec.trim();
        if spec.is_empty() {
            return Err(ParseError("is empty".into()));
        }
        if spec.contains('*') || spec.contains('!') {
            return Err(ParseError(
                "uses '*' or '!', which ringo does not support — write the \
                 cadence out with '+' and commas instead"
                    .into(),
            ));
        }
        let mut segments = Vec::new();
        let mut total_ms = 0u32;
        for (i, element) in spec.split(',').enumerate() {
            let nth = i + 1;
            let element = element.trim();
            let Some((freqs, ms)) = element.split_once('/') else {
                return Err(ParseError(format!(
                    "element {nth} ('{element}') has no duration — write it as freq/milliseconds"
                )));
            };
            let ms: u32 = ms.trim().parse().map_err(|_| {
                ParseError(format!(
                    "element {nth}: '{}' is not a duration in milliseconds",
                    ms.trim()
                ))
            })?;
            if ms == 0 {
                return Err(ParseError(format!("element {nth} lasts no time at all")));
            }
            total_ms = total_ms.saturating_add(ms);
            let freqs = parse_freqs(freqs.trim(), nth)?;
            segments.push(Segment { freqs, ms });
            if segments.len() > MAX_SEGMENTS {
                return Err(ParseError(format!("has more than {MAX_SEGMENTS} elements")));
            }
        }
        if total_ms > MAX_TOTAL_MS {
            return Err(ParseError(format!(
                "lasts {total_ms} ms, more than the {MAX_TOTAL_MS} ms a cadence may take"
            )));
        }
        Ok(Tone { segments })
    }
}

fn parse_freqs(spec: &str, nth: usize) -> Result<Vec<f64>, ParseError> {
    // A lone 0 is how a cadence writes its gap.
    if spec == "0" {
        return Ok(Vec::new());
    }
    let mut freqs = Vec::new();
    for part in spec.split('+') {
        let part = part.trim();
        let f: f64 = part
            .parse()
            .map_err(|_| ParseError(format!("element {nth}: '{part}' is not a frequency in Hz")))?;
        if !FREQ_RANGE.contains(&f) {
            return Err(ParseError(format!(
                "element {nth}: {f} Hz is outside {}–{} Hz",
                FREQ_RANGE.start(),
                FREQ_RANGE.end()
            )));
        }
        freqs.push(f);
        if freqs.len() > MAX_FREQS {
            return Err(ParseError(format!(
                "element {nth} mixes more than {MAX_FREQS} frequencies"
            )));
        }
    }
    Ok(freqs)
}

/// Why a tone spec could not be read. The message completes the sentence
/// "the tone …", so it reads as one line in a log.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

// ─── Synthesis ───────────────────────────────────────────────────────────────

/// Peak amplitude of a segment, as a fraction of full scale. Tones carry far
/// more energy than speech at the same peak, so this sits well below 1.0 — an
/// alert should be audible, not startling.
const LEVEL: f64 = 0.5;

/// Fade in and out of every tone segment. A sine cut mid-cycle is a step, and a
/// step is a click; a few milliseconds of raised cosine removes it without
/// audibly softening the cadence.
const FADE_MS: f64 = 5.0;

/// Render `periods` repetitions of `tone` as mono S16 samples at `srate`.
pub fn render(tone: &Tone, srate: u32, periods: u32) -> Vec<i16> {
    let mut out = Vec::new();
    for _ in 0..periods {
        for seg in &tone.segments {
            let n = (srate as u64 * seg.ms as u64 / 1000) as usize;
            if seg.freqs.is_empty() {
                out.extend(std::iter::repeat_n(0i16, n));
                continue;
            }
            // Split the level across the components so a dual tone peaks at the
            // same place a single one does.
            let amp = LEVEL / seg.freqs.len() as f64;
            let fade = ((srate as f64 * FADE_MS / 1000.0) as usize).min(n / 2);
            for i in 0..n {
                let t = i as f64 / srate as f64;
                let mut v = 0.0;
                for &f in &seg.freqs {
                    v += amp * (std::f64::consts::TAU * f * t).sin();
                }
                v *= envelope(i, n, fade);
                out.push((v * i16::MAX as f64).round().clamp(-32768.0, 32767.0) as i16);
            }
        }
    }
    out
}

/// Raised-cosine gain for sample `i` of `n`, ramping over `fade` samples at
/// each end.
fn envelope(i: usize, n: usize, fade: usize) -> f64 {
    if fade == 0 {
        return 1.0;
    }
    let rising = if i < fade {
        i as f64 / fade as f64
    } else {
        1.0
    };
    let falling = if i + fade >= n {
        (n - i) as f64 / fade as f64
    } else {
        1.0
    };
    let g = rising.min(falling).clamp(0.0, 1.0);
    0.5 - 0.5 * (std::f64::consts::PI * g).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRATE: u32 = 48000;

    fn parse(spec: &str) -> Tone {
        spec.parse()
            .unwrap_or_else(|e| panic!("'{spec}' should parse: {e}"))
    }

    #[test]
    fn every_built_in_spec_parses() {
        for spec in [DE_RINGBACK, DE_BUSY, DE_CONGESTION] {
            let t = parse(spec);
            assert_eq!(t.segments.len(), 2, "'{spec}' should be tone + gap");
            assert!(t.segments[1].freqs.is_empty(), "'{spec}' needs a gap");
        }
    }

    #[test]
    fn parses_a_single_frequency_cadence() {
        let t = parse("425/1000,0/4000");
        assert_eq!(t.segments[0].freqs, vec![425.0]);
        assert_eq!(t.segments[0].ms, 1000);
        assert_eq!(t.segments[1].freqs, Vec::<f64>::new());
        assert_eq!(t.segments[1].ms, 4000);
    }

    #[test]
    fn parses_a_mixed_frequency_cadence() {
        // The North American ringback, pasted from Asterisk's [us] zone.
        let t = parse("440+480/2000,0/4000");
        assert_eq!(t.segments[0].freqs, vec![440.0, 480.0]);
    }

    #[test]
    fn parses_the_british_double_ring() {
        // Four elements — the reason a tone is a segment list and not a triple.
        let t = parse("400+450/400,0/200,400+450/400,0/2000");
        assert_eq!(t.segments.len(), 4);
        assert_eq!(t.segments[2].freqs, vec![400.0, 450.0]);
    }

    #[test]
    fn tolerates_whitespace() {
        assert_eq!(parse(" 425/480 , 0/480 "), parse("425/480,0/480"));
    }

    #[test]
    fn rejects_a_missing_duration() {
        let e = "425".parse::<Tone>().unwrap_err().to_string();
        assert!(e.contains("no duration"), "unhelpful: {e}");
    }

    #[test]
    fn rejects_nonsense_numbers() {
        assert!("abc/500".parse::<Tone>().is_err());
        assert!("425/abc".parse::<Tone>().is_err());
        assert!("425/0".parse::<Tone>().is_err());
        assert!("".parse::<Tone>().is_err());
    }

    #[test]
    fn rejects_inaudible_frequencies() {
        assert!("2/500".parse::<Tone>().is_err());
        assert!("48000/500".parse::<Tone>().is_err());
    }

    #[test]
    fn names_the_asterisk_syntax_it_cannot_read() {
        // `!` and `*` appear in real indications.conf zones, so someone will
        // paste one. Saying which part is unsupported beats "invalid".
        let e = "!425/240,!0/240".parse::<Tone>().unwrap_err().to_string();
        assert!(e.contains('!'), "unhelpful: {e}");
        let e = "425*25/240".parse::<Tone>().unwrap_err().to_string();
        assert!(e.contains('*'), "unhelpful: {e}");
    }

    #[test]
    fn refuses_a_cadence_that_never_ends() {
        assert!("425/59000,0/59000".parse::<Tone>().is_err());
    }

    #[test]
    fn a_period_is_as_long_as_its_cadence() {
        let t = parse(DE_BUSY);
        let want = SRATE as usize * 960 / 1000;
        assert_eq!(render(&t, SRATE, 1).len(), want);
    }

    #[test]
    fn periods_repeat_exactly() {
        let t = parse(DE_BUSY);
        let one = render(&t, SRATE, 1);
        let three = render(&t, SRATE, 3);
        assert_eq!(three.len(), one.len() * 3);
        assert_eq!(&three[..one.len()], &one[..], "each period is identical");
    }

    #[test]
    fn the_gap_is_actually_silent() {
        let s = render(&parse(DE_BUSY), SRATE, 1);
        let gap = &s[SRATE as usize * 480 / 1000..];
        assert!(
            gap.iter().all(|&v| v == 0),
            "the cadence gap must be silent"
        );
    }

    #[test]
    fn segments_start_and_end_near_zero() {
        // The whole point of the envelope: no step at a segment boundary, so no
        // click when the cadence repeats.
        for spec in [DE_RINGBACK, DE_BUSY, DE_CONGESTION, "440+480/2000,0/4000"] {
            let s = render(&parse(spec), SRATE, 1);
            assert!(s[0].abs() < 100, "'{spec}' clicks on entry");
            assert!(s.last().unwrap().abs() < 100, "'{spec}' clicks on exit");
        }
    }

    #[test]
    fn stays_below_full_scale() {
        for spec in [DE_RINGBACK, "440+480/2000,0/4000"] {
            let peak = render(&parse(spec), SRATE, 1)
                .iter()
                .map(|v| v.abs())
                .max()
                .unwrap();
            assert!(peak > 1000, "'{spec}' is inaudibly quiet");
            assert!(
                (peak as f64) < LEVEL * 1.05 * i16::MAX as f64,
                "'{spec}' exceeds its level budget (peak {peak})"
            );
        }
    }

    #[test]
    fn a_single_tone_lands_on_its_frequency() {
        // Count zero crossings over the tone segment: a 425 Hz sine crosses zero
        // twice per cycle, so ~2 * 425 * 0.48 s over the 480 ms burst.
        let s = render(&parse(DE_BUSY), SRATE, 1);
        let burst = &s[..SRATE as usize * 480 / 1000];
        let crossings = burst
            .windows(2)
            .filter(|w| (w[0] < 0) != (w[1] < 0))
            .count();
        let expect = (2.0 * 425.0 * 0.48) as usize;
        assert!(
            crossings.abs_diff(expect) <= 2,
            "expected ~{expect} zero crossings, got {crossings}"
        );
    }
}
