
FROM ubuntu:24.04 AS builder
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    pkg-config \
    gcc \
    musl-tools \
    libclang-dev \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# flatc под версию flatbuffers-крейта
RUN curl -fsSL \
    "https://github.com/google/flatbuffers/releases/download/v25.12.19/Linux.flatc.binary.g%2B%2B-13.zip" \
    -o /tmp/flatc.zip \
    && unzip /tmp/flatc.zip -d /usr/local/bin \
    && chmod +x /usr/local/bin/flatc \
    && rm /tmp/flatc.zip

# Rust + musl-таргет
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /build
COPY . .

# Явно указываем компилятор/линкер для target-сборки (build.rs у blake3/lz4-sys/zstd-sys
# должны компилироваться именно под musl, а не под host gcc)
ENV CC_x86_64_unknown_linux_musl=musl-gcc \
    CXX_x86_64_unknown_linux_musl=musl-g++ \
    AR_x86_64_unknown_linux_musl=ar \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc \
    RUSTFLAGS="-C target-feature=+crt-static"

RUN cargo build --release --target x86_64-unknown-linux-musl \
    --no-default-features --features log-trace

# strip не нужен отдельно — profile.release уже содержит strip = true

# ---------------------------------------------------------------------------
# Stage 2 — Runtime (scratch)
# ---------------------------------------------------------------------------
FROM scratch

COPY --from=builder \
    /build/target/x86_64-unknown-linux-musl/release/{{project-name}} \
    /{{project-name}}

EXPOSE 8081

ENTRYPOINT ["/{{project-name}}"]
