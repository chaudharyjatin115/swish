use std::env;
use std::path::PathBuf;
use std::thread;
use swish_core::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = env::var("SWISH_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5357);

    let storage_dir = env::var("SWISH_DOWNLOAD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join("Downloads").join("Swish")
        });

    let device_name = env::var("SWISH_DEVICE_NAME").unwrap_or_else(|_| {
        let hostname = env::var("HOSTNAME").unwrap_or_else(|_| "Linux-Desktop".to_string());
        format!("Swish ({})", hostname)
    });

    let ephemeral_id = [0x53, 0x57, 0x49, 0x53, 0x48, 0x44, 0x01, 0x02];

    eprintln!("swishd: starting on 0.0.0.0:{port} ({device_name})");
    eprintln!("swishd: saving transfers to {}", storage_dir.display());

    let server = TransferServer::start(
        &format!("0.0.0.0:{}", port),
        storage_dir.clone(),
        ephemeral_id,
        device_name.clone(),
        |meta| {
            eprintln!(
                "swishd: incoming '{}' ({} bytes, {})",
                meta.name, meta.size_bytes, meta.mime_type
            );
            true
        },
        move |transfer| {
            eprintln!(
                "swishd: received '{}' -> {}",
                transfer.metadata.name,
                transfer.file_path.display()
            );
        },
    )?;

    // Publish mDNS service for zero-configuration LAN discovery (e.g. Android NsdDiscovery)
    let mdns_child = std::process::Command::new("avahi-publish-service")
        .args([&device_name, "_swish._tcp", &port.to_string()])
        .spawn()
        .ok();

    if mdns_child.is_some() {
        eprintln!("swishd: advertising mDNS service '_swish._tcp' on port {port}");
    }

    // Park main thread. Systemd and Unix signal handlers handle termination;
    // listener and worker threads service connections in the background.
    thread::park();

    if let Some(mut child) = mdns_child {
        let _ = child.kill();
    }
    drop(server);
    eprintln!("swishd: stopped.");
    Ok(())
}
