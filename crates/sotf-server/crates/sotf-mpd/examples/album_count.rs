use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <ip> <port>", args[0]);
        eprintln!("Example: {} 192.168.1.100 6600", args[0]);
        std::process::exit(1);
    }

    let ip = &args[1];
    let port = &args[2];
    let addr = format!("{}:{}", ip, port);

    println!("Connecting to MPD server at {}...", addr);
    let stream = TcpStream::connect(&addr).await?;
    println!("Connected!");

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Read greeting line
    let mut greeting = String::new();
    reader.read_line(&mut greeting).await?;
    println!("Server greeting: {}", greeting.trim());

    // Send list album command
    writer.write_all(b"list album\n").await?;
    writer.flush().await?;
    println!("Sent: list album");

    // Read response and count albums
    let mut album_count = 0;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }

        let line = line.trim();
        if line == "OK" {
            break;
        }
        if let Some(album_name) = line.strip_prefix("Album:").map(|s| s.trim()) {
            album_count += 1;
            println!("  Found album: {}", album_name);
        }
    }

    println!();
    println!("Total albums: {}", album_count);

    // Send close command
    writer.write_all(b"close\n").await?;
    writer.flush().await?;

    Ok(())
}
