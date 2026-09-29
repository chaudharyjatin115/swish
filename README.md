# Swish

Swish is an early-stage project for sending files between nearby devices inspire by huawei grab and share gesture. Right now this repo only has the Linux backend, written in Rust: a transfer library,a receiver daemon, and a command-line client. Android and camera support aren't in here yet and not funcational.

When a file comes in, the receiver writes the chunks to disk and checks them against the SHA-256 hash the sender provided before it reports the transfer as done. It can announce itself on your local network through Avahi, but the actual transfer happens over plain TCP.

## Heads up: what's not ready yet

- **No encryption or authentication.** Traffic goes over TCP in the clear, and the receiver doesn't verify who's sending. Please don't use Swish for anything sensitive or on a network you don't trust.
- **No approval step.** The daemon accepts every incoming transfer automatically. Nobody gets asked "do you want this file?"
- **The `camera` command won't work yet.** The CLI expects `linux/vision/camera_tracker.py`, which isn't part of this commit.
- **The hash check is limited.** SHA-256 catches corrupted or tampered files only if you can trust the hash the sender gave you. It doesn't prove who the sender is.

---

## What's in the repo

```text
swish/
├── linux/
│   ├── cli/                 # the swish command-line client
│   ├── daemon/              # swishd, the receiver daemon
│   └── library/             # framing and transfer code
├── protocol/                # wire protocol spec
├── README.md
├── LICENSE
└── CONTRIBUTING.md
```

---

## Building and testing

You'll need Rust installed. From the repo root:

```bash
cd linux

# Run the tests
cargo test --locked

# Build the release binaries
cargo build --release
```

You'll find the results in `linux/target/release/`:
- `swishd` is the background daemon
- `swish` is the command-line client

### Trying out the CLI

```bash
# See what's going on
./linux/target/release/swish status

# Start receiving files on port 5357, saving them to ./downloads
./linux/target/release/swish receive --port 5357 --dir ./downloads

# Send a file to another machine
./linux/target/release/swish send 192.168.1.50:5357 my_photo.jpg
```
