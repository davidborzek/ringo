# syntax=docker/dockerfile:1
#
# Minimal image for the ringo-flow scenario runner. Build from the repo root:
#   docker build -f crates/ringo-flow/Dockerfile -t ringo-flow .
#
# ringo-flow statically links libre + libbaresip (vendored) — no
# external baresip binary, no .so modules, no libre/libbaresip shared libs.
# OpenSSL is vendored (openssl-sys vendored) — no libssl runtime dep.
#
# Uses Debian (glibc) because Alpine/musl's dlopen cannot load libclang
# which bindgen needs at build time.

# ---- stage: compile ringo-flow (static baresip + vendored OpenSSL) ----
FROM rust:1-bookworm AS rust
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        cmake clang libclang-dev llvm-dev pkg-config perl \
        libspandsp-dev libopus-dev \
    && rm -rf /var/lib/apt/lists/*
ENV LIBCLANG_PATH=/usr/lib/llvm-14/lib
ENV CARGO_PROFILE_RELEASE_STRIP=true
WORKDIR /src
COPY . .
# bookworm's gcc emits aarch64 outline-atomics (e.g. __aarch64_ldadd4_acq_rel)
# into the static C deps (libre, OpenSSL), but rust links with -nodefaultlibs so
# libgcc (which defines them) isn't pulled in — and link order makes a trailing
# -lgcc ineffective. Build the C deps with -mno-outline-atomics so the symbols
# are never emitted. CFLAGS is honoured by both cmake (libre/baresip) and
# openssl-sys. arm64 only — amd64's gcc rejects the flag. TARGETARCH is set
# automatically by buildx for the target platform.
ARG TARGETARCH
RUN if [ "$TARGETARCH" = "arm64" ]; then export CFLAGS="-mno-outline-atomics"; fi; \
    cargo build --release -p ringo-flow --bin ringo-flow

# ---- runtime: binary + runtime deps (no libssl — vendored) ----
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates libspandsp2 libopus0 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=rust /src/target/release/ringo-flow /usr/local/bin/ringo-flow
WORKDIR /work
ENTRYPOINT ["ringo-flow"]
CMD ["--help"]
