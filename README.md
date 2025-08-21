# RFTPS - Rust FTP/FTPS Server

[![Build Status](https://github.com/hallowslab/rftps/actions/workflows/rust.yml/badge.svg)](https://github.com/hallowslab/rftps/actions/workflows/rust.yml/badge.svg)

[![Rust](https://skillicons.dev/icons?i=rust)](https://skillicons.dev)

A fast, secure, and lightweight FTP/FTPS server written in Rust. RFTPS provides an easy-to-use file transfer server with TLS encryption support and comprehensive logging.

## ✨ Features

- 🚀 **High Performance** - Built with Rust for speed and memory safety
- 🔒 **Secure** - FTPS support with TLS encryption
- 📁 **File Management** - Complete FTP operations (upload, download, delete, rename, mkdir)
- 🔐 **Authentication** - Simple username/password authentication
- 📊 **Logging** - Connection and data transfer logging
- ⚙️ **Configurable** - Flexible configuration via command-line arguments
- 🏠 **Auto Directory Creation** - Automatically creates specified directories
- 🎲 **Random Password Generation** - Auto-generates passwords if not provided

## 🚀 Quick Start

### Installation

```bash
# Or build from source
git clone https://github.com/yourusername/rftps.git
cd rftps
cargo build --release
```

### Basic Usage

```bash
# Start server with default settings
rftps

# Custom configuration
rftps --address 192.168.1.100 --port 2121 --directory ./my-ftp-root --username admin --password secret123
```

### With FTPS (TLS) Support

```bash
# Build with TLS support
cargo build --release --features include_pem_files

# Run with custom certificates
rftps --enable-ftps true --cert-pem ./cert.pem --key-pem ./key.pem
```

## 📋 Command Line Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--address` | `-a` | `0.0.0.0` | Server bind address |
| `--port` | `-p` | `21212` | FTP server port |
| `--directory` | `-d` | `./rftps` | Root directory for file storage |
| `--username` | `-u` | `rftps` | FTP username |
| `--password` | `-P` | *random* | FTP password (auto-generated if not provided) |
| `--enable-ftps` | `-f` | `true` | Enable/disable FTPS |
| `--cert-pem` | | `cert.pem` | TLS certificate file |
| `--key-pem` | | `key.pem` | TLS private key file |

## 🔧 Configuration Examples

### Development Server
```bash
rftps --address 127.0.0.1 --port 21212 --directory ./dev-files
```

### Production Server with FTPS
```bash
rftps --address 0.0.0.0 --port 21 --directory /var/ftp --username ftpuser --password SecurePass123 --cert-pem /etc/ssl/cert.pem --key-pem /etc/ssl/private.pem
```

### Custom Passive Port Range
The server automatically uses passive ports in the range `50000-65535` for data connections.

## 🔒 Security Features

- **Username Validation** - Only alphanumeric usernames allowed
- **Path Security** - Directory validation prevents invalid Windows/Unix paths
- **TLS Encryption** - Full FTPS support with certificate validation
- **Connection Logging** - All login attempts and file operations are logged

## 📁 Directory Structure

```
your-ftp-root/
├── uploaded-files/
├── user-data/
└── ...
```

The server will automatically create the specified root directory if it doesn't exist.

## 🛠️ Building from Source

### Prerequisites
- Rust 1.70 or later
- Cargo

### Build Commands

```bash
# Standard build
cargo build --release

# With TLS support
cargo build --release --features include_pem_files

# Development build with debug info
cargo build
```

### Running Tests
```bash
cargo test
```

## 🔍 Logging Output

The server provides detailed logging for all operations:

```
Server Init
        => Listening on 0.0.0.0:21212
Config:
        Host: 192.168.1.100
        Port: 21212
        Username: rftps
        Password: aB3xK9

User rftps logged in
User rftps uploaded file /documents/file.txt
User rftps downloaded file /documents/file.txt
User rftps logged out
```

## 🚦 Features

### Core Functionality
- ✅ FTP Protocol Support
- ✅ FTPS (FTP over TLS)
- ✅ File Upload/Download
- ✅ Directory Operations
- ✅ File/Directory Deletion
- ✅ File Renaming
- ✅ User Authentication

### Optional Features
- `include_pem_files` - Enables TLS certificate loading

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 🔗 Dependencies

- [libunftp](https://crates.io/crates/libunftp) - Core FTP server implementation
- [tokio](https://crates.io/crates/tokio) - Async runtime
- [clap](https://crates.io/crates/clap) - Command line argument parsing
- [rand](https://crates.io/crates/rand) - Random password generation

## 📞 Support

If you encounter any problems or have questions, please:
1. Check the [Issues](https://github.com/hallowslab/rftps/issues) page
2. Create a new issue if your problem isn't already reported
3. Provide as much detail as possible about your environment and the issue

---

<div align="center">
Made with ❤️ and 🦀 Rust
</div>