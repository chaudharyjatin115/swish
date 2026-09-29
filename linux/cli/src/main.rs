use std::env;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use swish_core::*;

fn print_usage() {
    println!("Swish - Nearby sharing CLI");
    println!();
    println!("Usage:");
    println!("  swish status                             Check local daemon availability");
    println!("  swish send <host:port> <file_path>       Send a file to target device");
    println!("  swish receive [--port P] [--dir D]       Wait and receive a single file");
    println!("  swish camera <file_path> [--target host] Use laptop camera gesture to throw file");
    println!();
    println!("Examples:");
    println!("  swish send 192.168.1.50:5357 photo.jpg");
    println!("  swish receive --port 5357 --dir ./downloads");
    println!("  swish camera document.pdf --target 192.168.31.221:5357");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "status" => {
            let port = env::var("SWISH_PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(5357);
            println!("Swish CLI v0.1.0");
            println!("Protocol: Swish Wire v1 (Port {port})");
            match TcpStream::connect_timeout(
                &SocketAddr::from(([127, 0, 0, 1], port)),
                Duration::from_millis(500),
            ) {
                Ok(_) => println!("Daemon: Running"),
                Err(error) => {
                    eprintln!("Daemon: Not running on 127.0.0.1:{port} ({error})");
                    std::process::exit(1);
                }
            }
        }
        "send" => {
            if args.len() < 4 {
                eprintln!("Error: 'send' requires target address and file path.");
                eprintln!("Usage: swish send <host:port> <file_path>");
                std::process::exit(1);
            }
            let target = &args[2];
            let file_path = Path::new(&args[3]);

            if !file_path.exists() {
                eprintln!("Error: File not found: {}", file_path.display());
                std::process::exit(1);
            }

            println!("Connecting to {}...", target);
            let client = TransferClient::new([1, 2, 3, 4, 5, 6, 7, 8], "Swish CLI".to_string());

            client.send_file(
                target,
                file_path,
                "application/octet-stream",
                |sent, total| {
                    let percent = sent.saturating_mul(100).checked_div(total).unwrap_or(100);
                    print!("\rSending: {}% ({} / {} bytes)", percent, sent, total);
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                },
            )?;

            println!("\nTransfer completed successfully!");
        }
        "receive" => {
            let mut port = 5357u16;
            let mut storage_dir = PathBuf::from("./downloads");

            let mut i = 2;
            while i < args.len() {
                if args[i] == "--port" && i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(5357);
                    i += 2;
                } else if args[i] == "--dir" && i + 1 < args.len() {
                    storage_dir = PathBuf::from(&args[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            }

            let hostname = env::var("HOSTNAME").unwrap_or_else(|_| "Linux-Laptop".to_string());
            let mdns = std::process::Command::new("avahi-publish-service")
                .args([
                    &format!("Swish-Linux ({})", hostname),
                    "_swish._tcp",
                    &port.to_string(),
                    "type=DESKTOP",
                ])
                .spawn()
                .ok();

            let (tx, rx) = mpsc::channel();
            let server = TransferServer::start(
                &format!("0.0.0.0:{}", port),
                storage_dir,
                [8, 7, 6, 5, 4, 3, 2, 1],
                "Swish Receiver".to_string(),
                |meta| {
                    println!(
                        "Accepting incoming file '{}' ({} bytes)",
                        meta.name, meta.size_bytes
                    );
                    true
                },
                move |transfer| {
                    let _ = tx.send(transfer);
                },
            )?;

            println!(
                "Waiting for incoming transfer on port {} (mDNS announced)...",
                port
            );
            let received = rx.recv_timeout(Duration::from_secs(120))?;
            println!(
                "Successfully received '{}' -> {}",
                received.metadata.name,
                received.file_path.display()
            );

            if let Some(mut child) = mdns {
                let _ = child.kill();
            }
            drop(server);
        }
        "camera" => {
            if args.len() < 3 {
                eprintln!("Error: 'camera' requires a file path to throw.");
                eprintln!("Usage: swish camera <file_path> [--target <host:port>]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let mut target: Option<&str> = None;
            let mut i = 3;
            while i < args.len() {
                if (args[i] == "--target" || args[i] == "-t") && i + 1 < args.len() {
                    target = Some(&args[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            }

            let tracker_script = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("vision")
                .join("camera_tracker.py");

            let mut cmd = std::process::Command::new("python3");
            cmd.arg(&tracker_script).arg(file_path);
            if let Some(t) = target {
                cmd.arg("--target").arg(t);
            }

            let status = cmd.status()?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        other => {
            eprintln!("Unknown command: '{}'", other);
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}
