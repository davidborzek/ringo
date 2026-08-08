FROM debian:bookworm-slim
# Retries: deb.debian.org resets connections mid-download often enough to fail a
# layer. apt output is kept so the next failure names its own cause.
RUN printf 'Acquire::Retries "5";\n' > /etc/apt/apt.conf.d/80-retries \
    && apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libspandsp2 libopus0 libpulse0 \
    && rm -rf /var/lib/apt/lists/*
COPY ringo /usr/local/bin/ringo
RUN ringo --help
