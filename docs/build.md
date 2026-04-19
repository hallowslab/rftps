# Building RFTPS

`rftps` is a high-performance FTP/FTPS server built on top of `libunftp`.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (Edition 2024 supported)

## Build Options

### Standard Build (No FTPS/PEM Embedding)

By default, the project builds without embedded PEM files. This is suitable if you intend to provide certificates at runtime via configuration or if you are only using plain FTP.

```bash
cargo build --release
```

### Build with Embedded PEM Files

To embed default PEM certificates (useful for development or standalone deployments where certificates are bundled into the binary), use the `include_pem_files` feature.

```bash
cargo build --release --features include_pem_files
```

**Note:** When this feature is enabled, the server will attempt to load `cert.pem` and `key.pem` from the project root at compile time and embed them.

## Usage after Build

The compiled binary will be located at `target/release/rftps`.
