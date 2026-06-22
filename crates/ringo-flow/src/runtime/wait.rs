//! `wait` with a call-drop guard: hold for a duration, but fail fast if a call
//! that is established at the start closes during it. During a wait the scenario
//! issues no commands, so such a close is an external (peer/network) drop.

use super::state::AgentState;
use anyhow::{Result, bail};
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

/// A snapshot of the agents' state-watch receivers, cloned by the caller *before*
/// releasing the sessions lock — so the lock isn't held across the `block_on`.
pub(crate) type Watchers = Vec<(String, watch::Receiver<AgentState>)>;

pub(crate) async fn wait_holding(duration: Duration, watchers: Watchers) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<(String, String)>(8);
    let mut guards = Vec::new();
    for (name, watch) in watchers {
        // One receiver, snapshot derived from it: the snapshot and the guard's
        // change baseline share a version, so a close folded in between can't be
        // missed (it would have to land between this `borrow()` and `changed()`,
        // and `guard_calls` re-checks the current state before awaiting).
        let established = watch.borrow().established_ids();
        if !established.is_empty() {
            guards.push(tokio::spawn(guard_calls(
                name,
                watch,
                established,
                tx.clone(),
            )));
        }
    }
    drop(tx);

    let outcome = tokio::select! {
        _ = tokio::time::sleep(duration) => Ok(()),
        msg = rx.recv() => match msg {
            Some((agent, reason)) => {
                bail!("call on `{agent}` dropped during wait — reason: {reason:?}")
            }
            None => Ok(()), // no calls to guard / all sessions ended
        },
    };
    for g in guards {
        g.abort();
    }
    outcome
}

async fn guard_calls(
    agent: String,
    mut rx: watch::Receiver<AgentState>,
    established: HashSet<String>,
    tx: mpsc::Sender<(String, String)>,
) {
    loop {
        let (current, reason) = {
            let s = rx.borrow();
            (s.call_ids(), s.last_call_reason.clone())
        };
        if established.iter().any(|id| !current.contains(id)) {
            let _ = tx.send((agent, reason.unwrap_or_default())).await;
            return;
        }
        if rx.changed().await.is_err() {
            return; // session ended, no drop observed
        }
    }
}
