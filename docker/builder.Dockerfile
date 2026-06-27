FROM debian:bullseye-slim
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends \
    cmake clang libclang-dev llvm-dev pkg-config make perl \
    libspandsp-dev libopus-dev libpulse-dev \
    curl ca-certificates \
    >/dev/null 2>&1 && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /work
