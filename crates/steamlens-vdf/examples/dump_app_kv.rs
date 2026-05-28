use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const RECORD_FIXED_HEADER_LEN: usize = 4 + 4 + 8 + 20 + 4 + 20;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: dump_app_kv <appid> [<appid> ...]");
        return ExitCode::from(2);
    }
    let wanted: std::collections::HashSet<u32> =
        args.iter().map(|s| s.parse::<u32>().unwrap()).collect();

    let path = PathBuf::from(env::var("HOME").expect("$HOME"))
        .join(".local/share/Steam/appcache/appinfo.vdf");
    let bytes = std::fs::read(&path).expect("read appinfo.vdf");

    let st_offset = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(bytes[st_offset..st_offset + 4].try_into().unwrap()) as usize;
    let mut strings = Vec::with_capacity(count);
    let mut p = st_offset + 4;
    for _ in 0..count {
        let nul = bytes[p..].iter().position(|&b| b == 0).unwrap();
        strings.push(String::from_utf8_lossy(&bytes[p..p + nul]).into_owned());
        p += nul + 1;
    }

    let mut pos = 16usize;
    loop {
        let app_id = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
        pos += 4;
        if app_id == 0 {
            break;
        }
        let size = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        let body_start = pos;
        let body_end = body_start + size;
        if wanted.contains(&app_id) {
            println!("\n=== app {app_id} (size={size}) ===");
            let blob_start = body_start + RECORD_FIXED_HEADER_LEN;
            walk(&bytes, blob_start, body_end, &strings);
        }
        pos = body_end;
    }

    ExitCode::SUCCESS
}

fn walk(data: &[u8], start: usize, end: usize, strings: &[String]) {
    let mut i = start;
    let mut depth = 0i32;
    let mut path: Vec<String> = Vec::new();
    while i < end {
        let tag = data[i];
        i += 1;
        if tag == 0x08 {
            if !path.is_empty() {
                path.pop();
            }
            depth -= 1;
            if depth < 0 {
                return;
            }
            continue;
        }
        if i + 4 > end {
            return;
        }
        let key_idx = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        let key = strings.get(key_idx).map(String::as_str).unwrap_or("<oob>");
        match tag {
            0x00 => {
                depth += 1;
                path.push(key.to_owned());
                let joined = path.join("/");
                let lower = joined.to_lowercase();
                if path.len() <= 3
                    || lower.contains("library")
                    || lower.contains("header")
                    || lower.contains("capsule")
                    || lower.contains("hero")
                    || lower.contains("logo")
                    || lower.contains("clienticon")
                {
                    println!("  [section] {joined}");
                }
            }
            0x01 => {
                let nul = data[i..].iter().position(|&b| b == 0).unwrap_or(0);
                let val = String::from_utf8_lossy(&data[i..i + nul]).into_owned();
                i += nul + 1;
                let joined = format!("{}/{}", path.join("/"), key);
                let lower = joined.to_lowercase();
                if lower.contains("library")
                    || lower.contains("header")
                    || lower.contains("capsule")
                    || lower.contains("hero")
                    || lower.contains("logo")
                    || lower.contains("clienticon")
                    || (path.len() <= 2 && val.len() < 80)
                {
                    println!("  [str] {joined} = \"{val}\"");
                }
            }
            0x02 | 0x03 | 0x04 | 0x06 => i += 4,
            0x07 | 0x09 | 0x0a => i += 8,
            _ => {
                println!("  <unknown tag 0x{tag:02x} at {i} — stopping app>");
                return;
            }
        }
    }
}
