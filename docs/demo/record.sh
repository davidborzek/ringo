#!/usr/bin/env bash
# Records docs/src/assets/ringo-phone.gif with VHS: alice is on screen; a
# Kamailio registrar plus bob and charlie run headless in the background, and
# each of them answers 2 s after a call to them starts ringing. Everything stays
# on loopback.
#
# Usage: docs/demo/record.sh [ringo-binary] [output.gif]
#   Defaults: ringo built from this checkout, docs/src/assets/ringo-phone.gif.
# Needs only nix (kamailio, vhs, ttyd and ffmpeg come from nixpkgs).
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
DIR=$PWD
REPO=$(git rev-parse --show-toplevel)
if [ $# -ge 1 ]; then
  RINGO=$(readlink -f "$1")
else
  (cd "$REPO" && nix develop -c cargo build -q -p ringo-phone)
  RINGO=$REPO/target/debug/ringo
fi
OUT=$(readlink -f "${2:-$REPO/docs/src/assets/ringo-phone.gif}")
# Scratch state outside the repo; recreated on every run.
RUN=${XDG_CACHE_HOME:-$HOME/.cache}/ringo-demo-gif

# Own ports so a running demo elsewhere doesn't clash.
REG_PORT=5260
declare -A PORT=([alice]=5271 [bob]=5272 [charlie]=5273)
CALLEES=(bob charlie)

rm -rf "$RUN"
mkdir -p "$RUN/kamailio" "$RUN/rt"
chmod 700 "$RUN/rt"
export XDG_RUNTIME_DIR=$RUN/rt

pids=()
cleanup() {
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  # alice runs inside VHS's terminal; an aborted recording can leave it
  # behind. Match by its demo HOME so no other ringo session is touched.
  for p in $(pgrep -U "$(id -u)" -x ringo); do
    if tr '\0' '\n' <"/proc/$p/environ" 2>/dev/null | grep -qx "HOME=$RUN/alice"; then
      kill "$p" 2>/dev/null || true
    fi
  done
  wait 2>/dev/null || true
}
cleanup
trap cleanup EXIT

# --- Registrar ---------------------------------------------------------------
sed -e "s/@REG_PORT@/$REG_PORT/" -e "s#@CTL@#$RUN/kamailio/ctl#" \
  "$DIR/kamailio.cfg.in" >"$RUN/kamailio/kamailio.cfg"
K=$(nix build --no-link --print-out-paths nixpkgs#kamailio)
"$K/bin/kamailio" -f "$RUN/kamailio/kamailio.cfg" -L "$K/lib/kamailio/modules" \
  -Y "$RUN/kamailio" -DD -E >"$RUN/kamailio.log" 2>&1 &
pids+=($!)

# --- Identities --------------------------------------------------------------
mk() { # name
  local cfg=$RUN/$1/.config/ringo
  mkdir -p "$cfg/profiles/$1"
  cat >"$cfg/ringo.toml" <<EOF
include = ["theme.toml"]

[baresip]
audio_driver = "aubridge"

[baresip.extra]
net_interface = "127.0.0.1"
sip_listen = "127.0.0.1:${PORT[$1]}"
EOF
  cp "$DIR/theme.toml" "$cfg/theme.toml"
  cat >"$cfg/profiles/$1/profile.toml" <<EOF
display_name = "${1^}"
username = "$1"
password = ""
domain = "demo.local"
outbound = "sip:127.0.0.1:$REG_PORT"
transport = "udp"
regint = 300
notify = false
mwi = false
EOF
}
for n in alice "${CALLEES[@]}"; do mk "$n"; done

# alice's contacts and picker: everyone else, so picking looks like it would.
# Only alice is started from it (the others run headless in their own HOME).
: >"$RUN/alice/.config/ringo/contacts.toml"
for n in "${CALLEES[@]}"; do
  printf '[[contacts]]\nname = "%s"\nnumbers = ["%s"]\n\n' "${n^}" "$n" \
    >>"$RUN/alice/.config/ringo/contacts.toml"
  cp -r "$RUN/$n/.config/ringo/profiles/$n" "$RUN/alice/.config/ringo/profiles/"
done

# --- Callees: headless, each answers 2 s after its call starts ringing -------
for n in "${CALLEES[@]}"; do
  HOME=$RUN/$n XDG_STATE_HOME=$RUN/$n/state \
    "$RINGO" start "$n" --headless --no-notify >"$RUN/$n.log" 2>&1 &
  pids+=($!)
  (
    ctl() { "$RINGO" control -t "$n" "$@" 2>/dev/null || true; }
    while :; do
      if ctl --json status | grep -q '"state": "ringing"'; then
        sleep 2
        ctl accept >/dev/null
        # Answer once per recording; the tape hangs up itself.
        break
      fi
      sleep 0.2
    done
  ) &
  pids+=($!)
done

# Wait until the callees are registered, so alice's calls find them.
for n in "${CALLEES[@]}"; do
  for _ in $(seq 1 50); do
    "$RINGO" control -t "$n" --json status 2>/dev/null |
      grep -q '"registration": "registered"' && break
    sleep 0.2
  done
done

# --- Record alice --------------------------------------------------------------
# `ringo` on the demo shell's PATH, so the GIF shows the plain command.
mkdir -p "$RUN/bin"
ln -sf "$RINGO" "$RUN/bin/ringo"
sed -e "s#@OUT@#$OUT#" -e "s#@BIN@#$RUN/bin#" -e "s#@HOME@#$RUN/alice#" \
  -e "s#@RT@#$XDG_RUNTIME_DIR#" "$DIR/ringo.tape.in" >"$RUN/ringo.tape"
nix shell nixpkgs#vhs nixpkgs#ttyd nixpkgs#ffmpeg -c vhs "$RUN/ringo.tape"
echo "wrote $OUT"
