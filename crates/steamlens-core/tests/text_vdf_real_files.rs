/// Requires a real Steam installation.  Run with:
///   cargo test -p steamlens-core --test text_vdf_real_files -- --ignored --nocapture
#[test]
#[ignore]
fn parse_real_libraryfolders_vdf() {
    let home = std::env::var("HOME").expect("HOME not set");
    let path = std::path::Path::new(&home).join(".local/share/Steam/steamapps/libraryfolders.vdf");

    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()));

    let root =
        steamlens_vdf::parse_text(&content).unwrap_or_else(|e| panic!("parse_text failed: {e}"));

    let lf = root
        .get("libraryfolders")
        .expect("missing top-level 'libraryfolders' key");

    let pairs = lf.as_block().expect("libraryfolders value is not a block");

    let paths: Vec<&str> = pairs
        .iter()
        .filter(|(k, _)| k.parse::<u64>().is_ok())
        .filter_map(|(_, v)| v.get("path")?.as_str())
        .collect();

    println!("Found {} library path(s):", paths.len());
    for p in &paths {
        println!("  {p}");
    }

    assert!(
        !paths.is_empty(),
        "expected at least one library path in libraryfolders.vdf"
    );
}
