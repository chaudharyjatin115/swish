# Swish Wire Protocol Specification

Version: 1 (Draft)

## Overview

The Swish Wire Protocol operates over reliable byte streams. The current Linux implementation uses TCP between nearby devices; TLS transport and peer authentication remain planned. The protocol is designed to be lightweight, streaming-first, and robust against truncated or corrupted frames.

---

## 1. Frame Structure

Every transmission consists of one or more frames formatted as:

```text
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|       'S'     |       'W'     |       'S'     |       'H'     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  Message Type |               Payload Length (Big-Endian)    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| ... Payload Length (cont.)    |                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+                               +
|                       Body Data (N Bytes)                     |
|                               ...                             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **Magic** (4 bytes): Constant ASCII bytes `0x53 0x57 0x53 0x48` (`"SWSH"`).
- **Message Type** (1 byte): Enumerated opcode identifying the frame purpose.
- **Payload Length** (4 bytes): Big-endian 32-bit unsigned integer denoting payload size. Max allowed length is 16 MiB (`16,777,216` bytes) per frame.
- **Body Data** (N bytes): Message payload corresponding to `Payload Length`.

---

## 2. Message Types

| Opcode | Identifier | Description |
| :--- | :--- | :--- |
| `0x01` | `HANDSHAKE_REQUEST` | Sender initiates session with ephemeral ID & device name |
| `0x02` | `HANDSHAKE_RESPONSE` | Receiver accepts/rejects handshake and returns its ephemeral ID |
| `0x10` | `TRANSFER_REQUEST` | Sender announces content metadata (name, mime, size) |
| `0x11` | `TRANSFER_ACCEPT` | Receiver authorizes the transmission |
| `0x12` | `TRANSFER_REJECT` | Receiver declines transmission with reason |
| `0x20` | `DATA_CHUNK` | Binary stream slice (typically 32 KB – 64 KB) |
| `0x21` | `TRANSFER_COMPLETE` | Finalization frame containing computed SHA-256 hex digest |
| `0x30` | `CANCEL` | Either peer cancels the active transfer |

---

## 3. String Encoding

Variable-length strings are encoded with a 2-byte big-endian unsigned length prefix followed by UTF-8 bytes (Java `DataOutputStream.writeUTF()` format).

---

## 4. Transfer Flow

```text
Sender                                              Receiver
  │                                                    │
  │─── HANDSHAKE_REQUEST (EphemeralId, DeviceName) ───>│
  │<── HANDSHAKE_RESPONSE (Accepted, EphemeralId) ─────│
  │                                                    │
  │─── TRANSFER_REQUEST (Id, Type, Name, Size) ───────>│
  │<── TRANSFER_ACCEPT (Id) ───────────────────────────│
  │                                                    │
  │─── DATA_CHUNK (Byte Slice) ───────────────────────>│
  │─── DATA_CHUNK (Byte Slice) ───────────────────────>│
  │                    ...                             │
  │─── TRANSFER_COMPLETE (Id, SHA256) ────────────────>│
  │                                                    │
  │ [Both peers close or keep alive for next payload]  │
```

---

## 5. Security & Validation Rules

1. **Path Traversal Protection**: Receivers must strip directory separators (`/`, `\`) from file names and sanitize invalid filesystem characters before saving.
2. **Integrity Verification**: Receivers stream incoming chunks into a running SHA-256 digest and compare the calculated checksum against `TRANSFER_COMPLETE.sha256Hex`. Any mismatch results in immediate deletion of the temporary file and an error.
3. **Cancellation Cleanup**: If an incomplete transfer is aborted or disconnected, receivers must delete the partial file to avoid leaving corrupt data on disk.
4. **Transport Security**: Plain TCP does not provide confidentiality or peer authentication. Implementations must not treat the handshake or SHA-256 digest as proof of peer identity; TLS and an authentication mechanism are required before sensitive-file use.
