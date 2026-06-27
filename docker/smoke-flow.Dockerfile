FROM debian:bookworm-slim
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends \
    ca-certificates libspandsp2 libopus0 \
    >/dev/null 2>&1 && rm -rf /var/lib/apt/lists/*
COPY ringo-flow /usr/local/bin/ringo-flow
RUN ringo-flow --help
