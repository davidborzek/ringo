# syntax=docker/dockerfile:1
#
# Minimal image for the ringo-flow scenario runner. Build from the repo root:
#   docker build -f crates/ringo-flow/Dockerfile -t ringo-flow .
#
# baresip is compiled from source — the distro packages are far too old (Debian
# 1.x, Alpine stable has none) for the >= 3.14 ringo needs, and a source build also
# lets us ship only the modules ringo-flow drives and add baresip-apps `redirect`
# (call deflection) later. Everything is built on Alpine/musl so the runtime image
# stays tiny: just baresip + its modules + the ringo-flow binary + a couple of libs.
#
# re (libre) and baresip are released in lockstep with the same version number, so
# pin both to the same tag for a reproducible build (bump them together).
ARG ALPINE=alpine:3.21
ARG RE_REF=v4.9.0
ARG BARESIP_REF=v4.9.0

# ---- stage: compile libre (re) + baresip ----
FROM ${ALPINE} AS baresip
RUN apk add --no-cache build-base cmake git openssl-dev libsndfile-dev linux-headers zlib-dev
ARG RE_REF
ARG BARESIP_REF
WORKDIR /b
# libre first; baresip links against it.
RUN git clone --depth 1 --branch "${RE_REF}" https://github.com/baresip/re.git \
 && cmake -S re -B re/build -DCMAKE_BUILD_TYPE=Release \
 && cmake --build re/build -j"$(nproc)" \
 && cmake --install re/build
RUN git clone --depth 1 --branch "${BARESIP_REF}" https://github.com/baresip/baresip.git \
 && cmake -S baresip -B baresip/build -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH=/usr/local \
 && cmake --build baresip/build -j"$(nproc)" \
 && cmake --install baresip/build
# Fail early if a module ringo-flow loads wasn't built (missing dep, etc.).
RUN for m in aubridge ausine aufile sndfile g711 auconv auresamp \
             stun turn ice srtp dtls_srtp uuid account menu netroam ctrl_tcp mwi; do \
      test -f "/usr/local/lib/baresip/modules/$m.so" \
        || { echo "FATAL: baresip module $m.so was not built"; exit 1; }; \
    done

# ---- stage: compile ringo-flow (musl) ----
FROM rust:1-alpine AS rust
# reqwest uses rustls (no OpenSSL), so no openssl headers are needed; `musl-dev`
# covers the C link step (rust:alpine already ships gcc). Let cargo strip.
RUN apk add --no-cache musl-dev
ENV CARGO_PROFILE_RELEASE_STRIP=true
WORKDIR /src
COPY . .
RUN cargo build --release -p ringo-flow --bin ringo-flow

# ---- runtime: baresip + modules + ringo-flow, nothing else ----
FROM ${ALPINE}
# Shared libs baresip's modules link against at runtime (TLS, recording).
RUN apk add --no-cache libsndfile openssl
COPY --from=baresip /usr/local/bin/baresip /usr/local/bin/baresip
COPY --from=baresip /usr/local/lib/libre.so* /usr/local/lib/
COPY --from=baresip /usr/local/lib/libbaresip.so* /usr/local/lib/
COPY --from=baresip /usr/local/lib/baresip/modules /usr/local/lib/baresip/modules
COPY --from=rust /src/target/release/ringo-flow /usr/local/bin/ringo-flow
WORKDIR /work
ENTRYPOINT ["ringo-flow"]
CMD ["--help"]

# ---- later: baresip-apps `redirect` module (call deflection / SIP 302) ----
# Add a stage that builds it against the libre/baresip from the `baresip` stage and
# drop redirect.so into the modules dir copied above:
#   FROM baresip AS apps
#   RUN git clone --depth 1 https://github.com/baresip/baresip-apps.git \
#    && cmake -S baresip-apps -B baresip-apps/build -DCMAKE_PREFIX_PATH=/usr/local \
#    && cmake --build baresip-apps/build --target redirect
#   # then in runtime: COPY --from=apps .../redirect.so /usr/local/lib/baresip/modules/
