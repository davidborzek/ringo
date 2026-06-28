//! Per-call RTP/RTCP media statistics (jitter, packet loss, RTT) + an estimated
//! MOS, read from baresip's audio stream. Drives audio-quality assertions in
//! ringo-flow and a live quality indicator in ringo-phone.
//!
//! RTCP reports arrive roughly every ~5s, so values are only meaningful a few
//! seconds into a call. We snapshot the last stats at call close (the call still
//! exists in that event), so a scenario can assert on them *after* hanging up.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::event::MediaStats;

use super::bindings::*;
use super::re_thread::on_re_thread;

/// Last stats per UA, snapshotted at call close so they outlive the call.
static LAST_STATS: OnceLock<Mutex<HashMap<usize, MediaStats>>> = OnceLock::new();

fn last_stats() -> &'static Mutex<HashMap<usize, MediaStats>> {
    LAST_STATS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Read the current RTCP stats off a call's audio stream. Must run on the RE
/// thread. `None` if the call/stream/stats aren't available yet.
unsafe fn read_call_stats(call: *mut Call) -> Option<MediaStats> {
    if call.is_null() {
        return None;
    }
    let audio = unsafe { call_audio(call) };
    if audio.is_null() {
        return None;
    }
    let strm = unsafe { audio_strm(audio) };
    if strm.is_null() {
        return None;
    }
    let stats = unsafe { stream_rtcp_stats(strm) };
    if stats.is_null() {
        return None;
    }
    let s = unsafe { &*stats };
    let rx_lost = s.rx.lost;
    let rx_total = unsafe { stream_metric_get_rx_n_packets(strm) } as i64;
    let denom = rx_total + rx_lost.max(0) as i64;
    let packet_loss_pct = if denom > 0 {
        rx_lost.max(0) as f64 / denom as f64 * 100.0
    } else {
        0.0
    };
    let jitter_ms = s.rx.jit as f64 / 1000.0;
    let rtt_ms = s.rtt as f64 / 1000.0;
    Some(MediaStats {
        rtt_ms,
        jitter_ms,
        rx_lost,
        packet_loss_pct,
        mos: estimate_mos(rtt_ms, jitter_ms, packet_loss_pct),
    })
}

/// Simplified ITU-T G.107 E-model MOS estimate from latency, jitter and loss.
fn estimate_mos(rtt_ms: f64, jitter_ms: f64, loss_pct: f64) -> f64 {
    // Effective latency folds in jitter (weighted) and a fixed codec/dejitter
    // allowance.
    let eff_latency = rtt_ms / 2.0 + 2.0 * jitter_ms + 10.0;
    let mut r = if eff_latency < 160.0 {
        93.2 - eff_latency / 40.0
    } else {
        93.2 - (eff_latency - 120.0) / 10.0
    };
    // Each percent of loss costs ~2.5 R-factor points.
    r -= 2.5 * loss_pct;
    if r < 0.0 {
        return 1.0;
    }
    let mos = 1.0 + 0.035 * r + r * (r - 60.0) * (100.0 - r) * 7e-6;
    mos.clamp(1.0, 4.5)
}

/// Snapshot a call's stats at close (called from the RE thread in the event
/// handler, while the call still exists), keyed by UA so they survive teardown.
pub(super) fn snapshot_on_close(ua: usize, call: *mut Call) {
    if let Some(stats) = unsafe { read_call_stats(call) } {
        last_stats()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(ua, stats);
    }
}

/// Drop a UA's stored snapshot (on session teardown).
pub(super) fn forget(ua: usize) {
    last_stats()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&ua);
}

/// Current media stats for `ua`: live from the active call, or the last
/// snapshot from a finished call. `None` if neither is available.
pub fn media_stats(ua: usize) -> Option<MediaStats> {
    let mut live = None;
    on_re_thread(|| {
        let call = unsafe { ua_call(ua as *mut Ua) };
        if !call.is_null() {
            live = unsafe { read_call_stats(call) };
        }
    });
    live.or_else(|| {
        last_stats()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&ua)
            .copied()
    })
}

#[cfg(test)]
mod tests {
    use super::estimate_mos;

    #[test]
    fn perfect_conditions_high_mos() {
        // No loss, tiny rtt/jitter → near the 4.4 ceiling for G.711-ish.
        let mos = estimate_mos(20.0, 2.0, 0.0);
        assert!(mos > 4.3, "expected high MOS, got {mos}");
    }

    #[test]
    fn loss_and_latency_degrade_mos() {
        let good = estimate_mos(20.0, 2.0, 0.0);
        let bad = estimate_mos(300.0, 60.0, 5.0);
        assert!(
            bad < good,
            "loss/latency should lower MOS ({bad} !< {good})"
        );
        assert!((1.0..=4.5).contains(&bad), "MOS in range, got {bad}");
    }

    #[test]
    fn extreme_loss_floors_at_one() {
        assert_eq!(estimate_mos(500.0, 200.0, 50.0), 1.0);
    }
}
