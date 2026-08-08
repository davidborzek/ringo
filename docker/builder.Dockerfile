FROM debian:bullseye-slim
# apt output is kept: when a mirror or a suite goes away, the exit code alone
# ("exit code: 100") says nothing, and the build log is the only place the
# reason ever shows up.
RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake clang libclang-dev llvm-dev pkg-config make perl \
    libspandsp-dev libopus-dev libpulse-dev \
    curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /work
