# Swish Linux Companion

This directory contains the Linux daemon (`swishd`), companion library, and CLI interface (`swish`).

## Structure
- `daemon/`: Background TCP transfer daemon that advertises `_swish._tcp` over mDNS using Avahi.
- `library/`: Core protocol serialization and stream socket handling.
- `cli/`: Command-line tool for querying status, nearby devices, and initiating transfers.

The Linux companion currently discovers peers over the local network using mDNS; file data is
transferred over TCP. Install Avahi's `avahi-publish-service` utility to make the daemon discoverable.
