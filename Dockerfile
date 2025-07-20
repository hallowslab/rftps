# Stage 1: Setup the builder environment using Debian-based Rust image
FROM rust:slim-bookworm AS builder

ARG UPX_VERSION \
    WITH_WINE

ENV UPX_VERSION=${UPX_VERSION:-5.0.0} \
    WITH_WINE=${WITH_WINE:-no}

# Install dependencies required for cross-compiling to Windows GNU target
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    cmake \
    gcc-mingw-w64-x86-64 \
    nasm \
    ninja-build \
    openssl xz-utils wget \  
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /rftps

# Get upx
RUN wget "https://github.com/upx/upx/releases/download/v$UPX_VERSION/upx-$UPX_VERSION-amd64_linux.tar.xz" \
    && mkdir -p upx

RUN tar -xf "upx-$UPX_VERSION-amd64_linux.tar.xz" --strip-components=1 -C ./upx/

# Add Rust targets for Windows GNU and Linux
RUN rustup target add x86_64-pc-windows-gnu
RUN rustup target add x86_64-unknown-linux-gnu

# Generate TLS certificates
RUN openssl req -x509 -newkey rsa:2048 -nodes \
    -keyout key.pem -out cert.pem -days 3650 \
    -subj '/CN=RFTPS/O=RFTPS/C=PT'

COPY . .

# Run tests 
RUN cargo test --all

# Compile Windows release with Ninja
RUN export CMAKE_GENERATOR=Ninja; \
    cargo build --target x86_64-pc-windows-gnu --release --features include_pem_files \
    && mv ./target/x86_64-pc-windows-gnu/release/rftps.exe ./ \
    && cargo clean

# Pack application with UPX
RUN ./upx/upx --best rftps.exe

# Compile Linux release
RUN cargo build --target x86_64-unknown-linux-gnu --release --features include_pem_files \
    && mv ./target/x86_64-unknown-linux-gnu/release/rftps ./ \
    && cargo clean
    
# Pack application with UPX
RUN ./upx/upx --best rftps

# # Stage 2: Create a minimal runtime image
FROM debian:bookworm-slim

# Optional: Install Wine to run Windows binaries in Docker (not necessary for production)
RUN if [ "$WITH_WINE" == "yes" ]; then \
    apt-get update && apt-get install -y --no-install-recommends wine \
    && rm -rf /var/lib/apt/lists/* \
    fi

WORKDIR /rftps

# Copy the certificate and key
COPY --from=builder /rftps/key.pem .
COPY --from=builder /rftps/cert.pem .

# Copy the binaries
COPY --from=builder /rftps/rftps ./rftpsbin
COPY --from=builder /rftps/rftps.exe .

# #
#COPY --from=builder /rftps/upx ./upx/

CMD [ "/bin/bash" ]
#CMD ["/usr/bin/local/rftps"]