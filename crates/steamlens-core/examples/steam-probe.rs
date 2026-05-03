fn main() {
    match steamlens_core::connect() {
        Ok(client) => println!("Steam ID: {}", client.steam_id()),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
