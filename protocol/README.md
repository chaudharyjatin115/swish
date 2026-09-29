# Swish Protocol Specification

This directory contains the wire protocol specification intended to be shared by the Android and Linux implementations.

## Planned Transports
- **Session Negotiation**: Bluetooth Low Energy (BLE) GATT characteristics.
- **Payload Stream (current Linux implementation)**: Plain TCP on local Wi-Fi / LAN.
- **Payload Stream (planned)**: TLS over TCP after peer authentication is defined.
