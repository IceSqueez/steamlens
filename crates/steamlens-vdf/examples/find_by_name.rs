use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const RECORD_FIXED_HEADER_LEN: usize = 4 + 4 + 8 + 20 + 4 + 20;

fn main() -> ExitCode {
    let needle = match env::args().nth(1) {
        Some(s) => s.to_lowercase(),
        None => {
            eprintln!("usage: find_by_name <substring>");
            return ExitCode::from(2);
        }
    };

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
        let blob_start = pos + RECORD_FIXED_HEADER_LEN;
        let body_end = pos + size;

        if let Some(name) = first_common_name(&bytes, blob_start, body_end, &strings)
            && name.to_lowercase().contains(&needle)
        {
            println!("{app_id}\t{name}");
        }
        pos = body_end;
    }
    ExitCode::SUCCESS
}

fn first_common_name(data: &[u8], start: usize, end: usize, strings: &[String]) -> Option<String> {
    let mut i = start;
    let mut depth = 0i32;
    let mut in_common = false;
    while i < end {
        let tag = data[i];
        i += 1;
        if tag == 0x08 {
            depth -= 1;
            if depth < 0 {
                return None;
            }
            if depth == 0 {
                in_common = false;
            }
            continue;
        }
        if i + 4 > end {
            return None;
        }
        let key_idx = u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        let key = strings.get(key_idx).map(String::as_str).unwrap_or("");
        match tag {
            0x00 => {
                depth += 1;
                if depth == 2 && key.eq_ignore_ascii_case("common") {
                    in_common = true;
                }
            }
            0x01 => {
                let nul = data[i..].iter().position(|&b| b == 0).unwrap_or(0);
                let val = String::from_utf8_lossy(&data[i..i + nul]).into_owned();
                i += nul + 1;
                if in_common && depth == 2 && key.eq_ignore_ascii_case("name") {
                    return Some(val);
                }
            }
            0x02 | 0x03 | 0x04 | 0x06 => i += 4,
            0x07 | 0x09 | 0x0a => i += 8,
            _ => return None,
        }
    }
    None
}
