FROM debian:bullseye-slim
# apt output is kept: when a mirror or a suite goes away, the exit code alone
# ("exit code: 100") says nothing, and the build log is the only place the
# reason ever shows up.
#
# Retries because deb.debian.org is a CDN that occasionally resets a connection
# mid-download, which fails the whole layer:
#   E: Failed to fetch .../libubsan1_10.2.1-6_amd64.deb
#      Error reading from server - read (104: Connection reset by peer)
RUN printf 'Acquire::Retries "5";\n' > /etc/apt/apt.conf.d/80-retries \
    && apt-get update && apt-get install -y --no-install-recommends \
    cmake clang libclang-dev llvm-dev pkg-config make perl \
    libspandsp-dev libopus-dev libpulse-dev \
    curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /work
