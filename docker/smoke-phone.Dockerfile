FROM debian:bookworm-slim
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends \
    ca-certificates libspandsp2 libopus0 libpulse0 \
    >/dev/null 2>&1 && rm -rf /var/lib/apt/lists/*
COPY ringo /usr/local/bin/ringo
RUN ringo --help
