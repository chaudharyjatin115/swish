# Swish

Swish is an early-stage nearby file-transfer project. The current Git snapshot contains the Linux Rust backend: a TCP transfer library, a daemon, and a command-line client. Android and camera work is not included in this commit yet.

The Rust receiver accepts a transfer request, writes incoming chunks to disk, and checks the sender-provided SHA-256 digest before reporting completion. Avahi can advertise the receiver on the local network; the transfer itself uses TCP.

## Current limitations

- TCP traffic is not encrypted, and peers are not authenticated. Do not use this for sensitive files or on untrusted networks.
- The daemon currently accepts every transfer request. It does not ask a person to approve each incoming file.
- The CLI's `camera` command expects `linux/vision/camera_tracker.py`, which is not included in this commit.
- SHA-256 detects accidental or malicious changes only when the sender's digest is trusted; it does not authenticate the sender.

---

## Project Structure

```text
swish/
├── linux/
│   ├── cli/                 # swish command-line client
│   ├── daemon/              # swishd receiver daemon
│   └── library/             # framing and transfer implementation
├── protocol/                # wire protocol specification
├── README.md
├── LICENSE
└── CONTRIBUTING.md
```

---

## Building & Testing

### Linux (Rust)

```bash
cd linux

# Run the workspace tests
cargo test --locked

# Build release binaries (swishd daemon and swish CLI)
cargo build --release
```

Binaries are placed in `linux/target/release/`:
- `swishd`: Background daemon
- `swish`: Command-line interface

#### Using the Linux CLI

```bash
# Check status
./linux/target/release/swish status

# Receive a file on port 5357
./linux/target/release/swish receive --port 5357 --dir ./downloads

# Send a file to another device
./linux/target/release/swish send 192.168.1.50:5357 my_photo.jpg
```

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
