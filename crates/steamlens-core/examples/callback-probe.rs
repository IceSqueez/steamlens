fn main() {
    let client = match steamlens_core::connect() {
        Ok(c) => {
            println!("Connected. Steam ID: {}", c.steam_id());
            c
        }
        Err(e) => {
            eprintln!("Failed to connect: {e}");
            std::process::exit(1);
        }
    };

    let duration = std::time::Duration::from_secs(3);
    let interval = std::time::Duration::from_millis(100);
    let start = std::time::Instant::now();
    let mut total = 0usize;

    while start.elapsed() < duration {
        match client.poll_callbacks() {
            Ok(callbacks) => {
                for cb in &callbacks {
                    println!("id={} size={} bytes", cb.id, cb.payload.len());
                    total += 1;
                }
            }
            Err(e) => {
                eprintln!("poll_callbacks error: {e}");
                std::process::exit(1);
            }
        }
        std::thread::sleep(interval);
    }

    println!("Done. Received {total} callback(s) over 3 seconds.");
}
