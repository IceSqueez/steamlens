use std::collections::HashMap;

use thiserror::Error;

const MAGIC_CSTRING_V1: u32 = 0x0756_4427;
const MAGIC_CSTRING_V2: u32 = 0x0756_4428;
const MAGIC_STRING_TABLE: u32 = 0x0756_4429;

const RECORD_FIXED_HEADER_LEN: usize = 4 + 4 + 8 + 20 + 4 + 20;

/// Steam library image asset. `Hashed` means the value in appinfo is
/// `{40-hex-sha1}/{filename}` and the CDN URL nests under that hash directory;
/// `Plain` means the value is just `{filename}` and the CDN URL omits the hash.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageAsset {
    Hashed { hash: String, filename: String },
    Plain { filename: String },
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct AppLibraryAssets {
    pub cover: Option<ImageAsset>,
    pub background: Option<ImageAsset>,
    pub logo: Option<ImageAsset>,
    pub wide_cover: Option<ImageAsset>,
    pub wide_cover_legacy: Option<ImageAsset>,
}

pub fn parse_appinfo_assets(bytes: &[u8]) -> Result<HashMap<u32, AppLibraryAssets>, AppInfoError> {
    if bytes.len() < 16 {
        return Err(AppInfoError::Truncated {
            context: "file header",
        });
    }

    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

    match magic {
        MAGIC_CSTRING_V1 | MAGIC_CSTRING_V2 => parse_cstring_kv_assets(bytes, 8, magic),
        MAGIC_STRING_TABLE => parse_string_table_kv_assets(bytes),
        other => Err(AppInfoError::UnsupportedMagic { magic: other }),
    }
}

#[derive(Debug, Error)]
pub enum AppInfoError {
    #[error("appinfo.vdf magic 0x{magic:08x} is not supported")]
    UnsupportedMagic { magic: u32 },

    #[error("appinfo.vdf is truncated at {context}")]
    Truncated { context: &'static str },

    #[error("appinfo.vdf contains invalid UTF-8: {0}")]
    InvalidString(String),
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct AppFlags {
    pub visibility: Option<String>,
    pub has_store_asset_mtime: bool,
    pub has_library_assets: bool,
    pub has_header_image: bool,
}

pub fn parse_appinfo_flags(bytes: &[u8]) -> Result<HashMap<u32, AppFlags>, AppInfoError> {
    if bytes.len() < 16 {
        return Err(AppInfoError::Truncated {
            context: "file header",
        });
    }

    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

    match magic {
        MAGIC_CSTRING_V1 | MAGIC_CSTRING_V2 => parse_cstring_kv(bytes, 8, magic),
        MAGIC_STRING_TABLE => parse_string_table_kv(bytes),
        other => Err(AppInfoError::UnsupportedMagic { magic: other }),
    }
}

fn parse_cstring_kv(
    bytes: &[u8],
    header_len: usize,
    magic: u32,
) -> Result<HashMap<u32, AppFlags>, AppInfoError> {
    let fixed_header = if magic >= MAGIC_CSTRING_V2 {
        RECORD_FIXED_HEADER_LEN
    } else {
        RECORD_FIXED_HEADER_LEN - 20
    };

    let mut map = HashMap::new();
    let mut pos = header_len;

    loop {
        if pos + 4 > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record app_id",
            });
        }
        let app_id =
            u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
        pos += 4;

        if app_id == 0 {
            break;
        }

        if pos + 4 > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record size",
            });
        }
        let size = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
            as usize;
        pos += 4;

        let body_start = pos;
        let body_end = body_start.saturating_add(size);
        if body_end > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record header (v1/v2)",
            });
        }

        if size < fixed_header {
            pos = body_end;
            continue;
        }

        let blob = &bytes[body_start + fixed_header..body_end];
        if let Some(flags) = scan_cstring_blob(blob)? {
            map.insert(app_id, flags);
        }

        pos = body_end;
    }

    Ok(map)
}

fn parse_string_table_kv(bytes: &[u8]) -> Result<HashMap<u32, AppFlags>, AppInfoError> {
    if bytes.len() < 16 {
        return Err(AppInfoError::Truncated {
            context: "string-table file header",
        });
    }

    let st_offset = i64::from_le_bytes([
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    ]);

    if st_offset < 0 || st_offset as usize >= bytes.len() {
        return Err(AppInfoError::Truncated {
            context: "string table offset out of range",
        });
    }

    let strings = read_string_table(bytes, st_offset as usize)?;
    let mut map = HashMap::new();
    let mut pos = 16usize;

    loop {
        if pos + 4 > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record app_id (string-table)",
            });
        }
        let app_id =
            u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
        pos += 4;

        if app_id == 0 {
            break;
        }

        if pos + 4 > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record size (string-table)",
            });
        }
        let size = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
            as usize;
        pos += 4;

        let body_start = pos;
        let body_end = body_start.saturating_add(size);
        if body_end > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record body (string-table)",
            });
        }

        if size < RECORD_FIXED_HEADER_LEN {
            pos = body_end;
            continue;
        }

        let blob = &bytes[body_start + RECORD_FIXED_HEADER_LEN..body_end];
        if let Some(flags) = scan_indexed_blob(blob, &strings)? {
            map.insert(app_id, flags);
        }

        pos = body_end;
    }

    Ok(map)
}

fn parse_cstring_kv_assets(
    bytes: &[u8],
    header_len: usize,
    magic: u32,
) -> Result<HashMap<u32, AppLibraryAssets>, AppInfoError> {
    let fixed_header = if magic >= MAGIC_CSTRING_V2 {
        RECORD_FIXED_HEADER_LEN
    } else {
        RECORD_FIXED_HEADER_LEN - 20
    };

    let mut map = HashMap::new();
    let mut pos = header_len;

    loop {
        if pos + 4 > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record app_id (assets cstring)",
            });
        }
        let app_id =
            u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
        pos += 4;

        if app_id == 0 {
            break;
        }

        if pos + 4 > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record size (assets cstring)",
            });
        }
        let size = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
            as usize;
        pos += 4;

        let body_start = pos;
        let body_end = body_start.saturating_add(size);
        if body_end > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record body (assets cstring)",
            });
        }

        if size >= fixed_header {
            let blob = &bytes[body_start + fixed_header..body_end];
            if let Some(assets) = scan_cstring_blob_assets(blob)? {
                map.insert(app_id, assets);
            }
        }

        pos = body_end;
    }

    Ok(map)
}

fn parse_string_table_kv_assets(
    bytes: &[u8],
) -> Result<HashMap<u32, AppLibraryAssets>, AppInfoError> {
    if bytes.len() < 16 {
        return Err(AppInfoError::Truncated {
            context: "string-table file header (assets)",
        });
    }

    let st_offset = i64::from_le_bytes([
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    ]);

    if st_offset < 0 || st_offset as usize >= bytes.len() {
        return Err(AppInfoError::Truncated {
            context: "string table offset out of range (assets)",
        });
    }

    let strings = read_string_table(bytes, st_offset as usize)?;
    let mut map = HashMap::new();
    let mut pos = 16usize;

    loop {
        if pos + 4 > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record app_id (assets string-table)",
            });
        }
        let app_id =
            u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
        pos += 4;

        if app_id == 0 {
            break;
        }

        if pos + 4 > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record size (assets string-table)",
            });
        }
        let size = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
            as usize;
        pos += 4;

        let body_start = pos;
        let body_end = body_start.saturating_add(size);
        if body_end > bytes.len() {
            return Err(AppInfoError::Truncated {
                context: "app record body (assets string-table)",
            });
        }

        if size >= RECORD_FIXED_HEADER_LEN {
            let blob = &bytes[body_start + RECORD_FIXED_HEADER_LEN..body_end];
            if let Some(assets) = scan_indexed_blob_assets(blob, &strings)? {
                map.insert(app_id, assets);
            }
        }

        pos = body_end;
    }

    Ok(map)
}

fn read_string_table(bytes: &[u8], offset: usize) -> Result<Vec<String>, AppInfoError> {
    if offset + 4 > bytes.len() {
        return Err(AppInfoError::Truncated {
            context: "string table count",
        });
    }
    let count = u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]) as usize;

    let mut strings = Vec::with_capacity(count);
    let mut pos = offset + 4;

    for _ in 0..count {
        let nul = bytes[pos..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(AppInfoError::Truncated {
                context: "string table entry",
            })?;
        let s = std::str::from_utf8(&bytes[pos..pos + nul])
            .map_err(|e| AppInfoError::InvalidString(e.to_string()))?;
        strings.push(s.to_owned());
        pos += nul + 1;
    }

    Ok(strings)
}

fn parse_asset_value(value: &[u8]) -> Option<ImageAsset> {
    if value.is_empty() {
        return None;
    }
    let s = std::str::from_utf8(value).ok()?;
    if let Some(slash) = s.find('/') {
        let prefix = &s[..slash];
        let tail = &s[slash + 1..];
        if prefix.len() == 40 && prefix.chars().all(|c| c.is_ascii_hexdigit()) && !tail.is_empty() {
            return Some(ImageAsset::Hashed {
                hash: prefix.to_ascii_lowercase(),
                filename: tail.to_owned(),
            });
        }
    }
    Some(ImageAsset::Plain {
        filename: s.to_owned(),
    })
}

fn emit_assets(
    mut assets: AppLibraryAssets,
    found_laf: bool,
    header_image_value: Option<ImageAsset>,
) -> Option<AppLibraryAssets> {
    assets.wide_cover_legacy = header_image_value;
    if found_laf || assets.wide_cover_legacy.is_some() {
        Some(assets)
    } else {
        None
    }
}

fn scan_cstring_blob_assets(blob: &[u8]) -> Result<Option<AppLibraryAssets>, AppInfoError> {
    let mut i = 0usize;
    let mut depth: i32 = 0;
    let mut in_common = false;
    let mut common_depth: i32 = -1;
    let mut in_laf = false;
    let mut laf_depth: i32 = -1;
    let mut in_slot = false;
    let mut slot_depth: i32 = -1;
    let mut current_slot: u8 = 0;
    let mut in_image = false;
    let mut image_depth: i32 = -1;
    let mut in_header_image = false;
    let mut header_image_depth: i32 = -1;
    let mut header_image_value: Option<ImageAsset> = None;
    let mut assets = AppLibraryAssets::default();
    let mut found_laf = false;

    const SLOT_COVER: u8 = 1;
    const SLOT_BACKGROUND: u8 = 2;
    const SLOT_LOGO: u8 = 3;
    const SLOT_WIDE_COVER: u8 = 4;

    while i < blob.len() {
        let tag = blob[i];
        i += 1;

        if tag == 0x08 {
            if in_image && depth == image_depth {
                in_image = false;
                image_depth = -1;
            } else if in_slot && depth == slot_depth {
                in_slot = false;
                slot_depth = -1;
                current_slot = 0;
            } else if in_laf && depth == laf_depth {
                in_laf = false;
                laf_depth = -1;
            } else if in_header_image && depth == header_image_depth {
                in_header_image = false;
                header_image_depth = -1;
            } else if in_common && depth == common_depth {
                return Ok(emit_assets(assets, found_laf, header_image_value));
            }
            depth -= 1;
            if depth < 0 {
                return Ok(emit_assets(assets, found_laf, header_image_value));
            }
            continue;
        }

        let key_bytes = read_cstring_bytes(blob, &mut i).ok_or(AppInfoError::Truncated {
            context: "cstring key in assets blob",
        })?;

        match tag {
            0x00 => {
                depth += 1;
                if !in_common && key_bytes.eq_ignore_ascii_case(b"common") {
                    in_common = true;
                    common_depth = depth;
                } else if in_common && !in_laf && !in_header_image && depth == common_depth + 1 {
                    if key_bytes.eq_ignore_ascii_case(b"library_assets_full") {
                        in_laf = true;
                        laf_depth = depth;
                        found_laf = true;
                    } else if key_bytes.eq_ignore_ascii_case(b"header_image") {
                        in_header_image = true;
                        header_image_depth = depth;
                    }
                } else if in_laf && !in_slot && depth == laf_depth + 1 {
                    current_slot = if key_bytes.eq_ignore_ascii_case(b"library_capsule") {
                        SLOT_COVER
                    } else if key_bytes.eq_ignore_ascii_case(b"library_hero") {
                        SLOT_BACKGROUND
                    } else if key_bytes.eq_ignore_ascii_case(b"library_logo") {
                        SLOT_LOGO
                    } else if key_bytes.eq_ignore_ascii_case(b"library_header") {
                        SLOT_WIDE_COVER
                    } else {
                        0
                    };
                    if current_slot != 0 {
                        in_slot = true;
                        slot_depth = depth;
                    }
                } else if in_slot
                    && !in_image
                    && depth == slot_depth + 1
                    && key_bytes.eq_ignore_ascii_case(b"image")
                {
                    in_image = true;
                    image_depth = depth;
                }
            }
            0x01 => {
                let val_bytes =
                    read_cstring_bytes(blob, &mut i).ok_or(AppInfoError::Truncated {
                        context: "cstring value in assets blob",
                    })?;
                if in_image && depth == image_depth && key_bytes.eq_ignore_ascii_case(b"english") {
                    let slot_ref = match current_slot {
                        SLOT_COVER => Some(&mut assets.cover),
                        SLOT_BACKGROUND => Some(&mut assets.background),
                        SLOT_LOGO => Some(&mut assets.logo),
                        SLOT_WIDE_COVER => Some(&mut assets.wide_cover),
                        _ => None,
                    };
                    if let Some(dest) = slot_ref
                        && dest.is_none()
                    {
                        *dest = parse_asset_value(val_bytes);
                    }
                } else if in_header_image
                    && depth == header_image_depth
                    && key_bytes.eq_ignore_ascii_case(b"english")
                    && header_image_value.is_none()
                {
                    header_image_value = parse_asset_value(val_bytes);
                }
            }
            0x02 | 0x03 | 0x04 | 0x06 => {
                if i + 4 > blob.len() {
                    return Err(AppInfoError::Truncated {
                        context: "4-byte value in assets blob",
                    });
                }
                i += 4;
            }
            0x07 | 0x09 | 0x0a => {
                if i + 8 > blob.len() {
                    return Err(AppInfoError::Truncated {
                        context: "8-byte value in assets blob",
                    });
                }
                i += 8;
            }
            _ => return Ok(emit_assets(assets, found_laf, header_image_value)),
        }
    }

    Ok(emit_assets(assets, found_laf, header_image_value))
}

fn scan_indexed_blob_assets(
    blob: &[u8],
    strings: &[String],
) -> Result<Option<AppLibraryAssets>, AppInfoError> {
    let mut i = 0usize;
    let mut depth: i32 = 0;
    let mut in_common = false;
    let mut common_depth: i32 = -1;
    let mut in_laf = false;
    let mut laf_depth: i32 = -1;
    let mut in_slot = false;
    let mut slot_depth: i32 = -1;
    let mut current_slot: u8 = 0;
    let mut in_image = false;
    let mut image_depth: i32 = -1;
    let mut in_header_image = false;
    let mut header_image_depth: i32 = -1;
    let mut header_image_value: Option<ImageAsset> = None;
    let mut assets = AppLibraryAssets::default();
    let mut found_laf = false;

    const SLOT_COVER: u8 = 1;
    const SLOT_BACKGROUND: u8 = 2;
    const SLOT_LOGO: u8 = 3;
    const SLOT_WIDE_COVER: u8 = 4;

    while i < blob.len() {
        let tag = blob[i];
        i += 1;

        if tag == 0x08 {
            if in_image && depth == image_depth {
                in_image = false;
                image_depth = -1;
            } else if in_slot && depth == slot_depth {
                in_slot = false;
                slot_depth = -1;
                current_slot = 0;
            } else if in_laf && depth == laf_depth {
                in_laf = false;
                laf_depth = -1;
            } else if in_header_image && depth == header_image_depth {
                in_header_image = false;
                header_image_depth = -1;
            } else if in_common && depth == common_depth {
                return Ok(emit_assets(assets, found_laf, header_image_value));
            }
            depth -= 1;
            if depth < 0 {
                return Ok(emit_assets(assets, found_laf, header_image_value));
            }
            continue;
        }

        if i + 4 > blob.len() {
            return Err(AppInfoError::Truncated {
                context: "key index in assets indexed blob",
            });
        }
        let key_idx = u32::from_le_bytes([blob[i], blob[i + 1], blob[i + 2], blob[i + 3]]) as usize;
        i += 4;
        let key = strings.get(key_idx).map(String::as_str).unwrap_or("");

        match tag {
            0x00 => {
                depth += 1;
                if !in_common && key.eq_ignore_ascii_case("common") {
                    in_common = true;
                    common_depth = depth;
                } else if in_common && !in_laf && !in_header_image && depth == common_depth + 1 {
                    if key.eq_ignore_ascii_case("library_assets_full") {
                        in_laf = true;
                        laf_depth = depth;
                        found_laf = true;
                    } else if key.eq_ignore_ascii_case("header_image") {
                        in_header_image = true;
                        header_image_depth = depth;
                    }
                } else if in_laf && !in_slot && depth == laf_depth + 1 {
                    current_slot = if key.eq_ignore_ascii_case("library_capsule") {
                        SLOT_COVER
                    } else if key.eq_ignore_ascii_case("library_hero") {
                        SLOT_BACKGROUND
                    } else if key.eq_ignore_ascii_case("library_logo") {
                        SLOT_LOGO
                    } else if key.eq_ignore_ascii_case("library_header") {
                        SLOT_WIDE_COVER
                    } else {
                        0
                    };
                    if current_slot != 0 {
                        in_slot = true;
                        slot_depth = depth;
                    }
                } else if in_slot
                    && !in_image
                    && depth == slot_depth + 1
                    && key.eq_ignore_ascii_case("image")
                {
                    in_image = true;
                    image_depth = depth;
                }
            }
            0x01 => {
                let val_bytes =
                    read_cstring_bytes(blob, &mut i).ok_or(AppInfoError::Truncated {
                        context: "cstring value in assets indexed blob",
                    })?;
                if in_image && depth == image_depth && key.eq_ignore_ascii_case("english") {
                    let slot_ref = match current_slot {
                        SLOT_COVER => Some(&mut assets.cover),
                        SLOT_BACKGROUND => Some(&mut assets.background),
                        SLOT_LOGO => Some(&mut assets.logo),
                        SLOT_WIDE_COVER => Some(&mut assets.wide_cover),
                        _ => None,
                    };
                    if let Some(dest) = slot_ref
                        && dest.is_none()
                    {
                        *dest = parse_asset_value(val_bytes);
                    }
                } else if in_header_image
                    && depth == header_image_depth
                    && key.eq_ignore_ascii_case("english")
                    && header_image_value.is_none()
                {
                    header_image_value = parse_asset_value(val_bytes);
                }
            }
            0x02 | 0x03 | 0x04 | 0x06 => {
                if i + 4 > blob.len() {
                    return Err(AppInfoError::Truncated {
                        context: "4-byte value in assets indexed blob",
                    });
                }
                i += 4;
            }
            0x07 | 0x09 | 0x0a => {
                if i + 8 > blob.len() {
                    return Err(AppInfoError::Truncated {
                        context: "8-byte value in assets indexed blob",
                    });
                }
                i += 8;
            }
            _ => return Ok(emit_assets(assets, found_laf, header_image_value)),
        }
    }

    Ok(emit_assets(assets, found_laf, header_image_value))
}

fn scan_cstring_blob(blob: &[u8]) -> Result<Option<AppFlags>, AppInfoError> {
    let mut i = 0usize;
    let mut depth: i32 = 0;
    let mut in_common = false;
    let mut common_depth: i32 = -1;
    let mut flags = AppFlags::default();
    let mut found_common = false;

    while i < blob.len() {
        let tag = blob[i];
        i += 1;

        if tag == 0x08 {
            if in_common && depth == common_depth {
                return Ok(if found_common { Some(flags) } else { None });
            }
            depth -= 1;
            if depth < 0 {
                return Ok(if found_common { Some(flags) } else { None });
            }
            continue;
        }

        let key_bytes = read_cstring_bytes(blob, &mut i).ok_or(AppInfoError::Truncated {
            context: "cstring key in blob",
        })?;

        match tag {
            0x00 => {
                depth += 1;
                if !in_common && key_bytes.eq_ignore_ascii_case(b"common") {
                    in_common = true;
                    common_depth = depth;
                    found_common = true;
                } else if in_common && depth == common_depth + 1 {
                    if key_bytes.eq_ignore_ascii_case(b"library_assets") {
                        flags.has_library_assets = true;
                    } else if key_bytes.eq_ignore_ascii_case(b"header_image") {
                        flags.has_header_image = true;
                    }
                }
            }
            0x01 => {
                let val_bytes =
                    read_cstring_bytes(blob, &mut i).ok_or(AppInfoError::Truncated {
                        context: "cstring value in blob",
                    })?;
                if in_common && depth == common_depth {
                    if (key_bytes.eq_ignore_ascii_case(b"visibility")
                        || key_bytes.eq_ignore_ascii_case(b"section_type"))
                        && !val_bytes.is_empty()
                    {
                        flags.visibility =
                            Some(String::from_utf8_lossy(val_bytes).to_ascii_lowercase());
                    } else if key_bytes.eq_ignore_ascii_case(b"store_asset_mtime") {
                        flags.has_store_asset_mtime = true;
                    }
                }
            }
            0x02 | 0x03 | 0x04 | 0x06 => {
                if in_common
                    && depth == common_depth
                    && key_bytes.eq_ignore_ascii_case(b"store_asset_mtime")
                {
                    flags.has_store_asset_mtime = true;
                }
                if i + 4 > blob.len() {
                    return Err(AppInfoError::Truncated {
                        context: "4-byte value in blob",
                    });
                }
                i += 4;
            }
            0x07 | 0x09 | 0x0a => {
                if in_common
                    && depth == common_depth
                    && key_bytes.eq_ignore_ascii_case(b"store_asset_mtime")
                {
                    flags.has_store_asset_mtime = true;
                }
                if i + 8 > blob.len() {
                    return Err(AppInfoError::Truncated {
                        context: "8-byte value in blob",
                    });
                }
                i += 8;
            }
            _ => return Ok(if found_common { Some(flags) } else { None }),
        }
    }

    Ok(if found_common { Some(flags) } else { None })
}

fn scan_indexed_blob(blob: &[u8], strings: &[String]) -> Result<Option<AppFlags>, AppInfoError> {
    let mut i = 0usize;
    let mut depth: i32 = 0;
    let mut in_common = false;
    let mut common_depth: i32 = -1;
    let mut flags = AppFlags::default();
    let mut found_common = false;

    while i < blob.len() {
        let tag = blob[i];
        i += 1;

        if tag == 0x08 {
            if in_common && depth == common_depth {
                return Ok(if found_common { Some(flags) } else { None });
            }
            depth -= 1;
            if depth < 0 {
                return Ok(if found_common { Some(flags) } else { None });
            }
            continue;
        }

        if i + 4 > blob.len() {
            return Err(AppInfoError::Truncated {
                context: "key index in indexed blob",
            });
        }
        let key_idx = u32::from_le_bytes([blob[i], blob[i + 1], blob[i + 2], blob[i + 3]]) as usize;
        i += 4;

        let key = strings.get(key_idx).map(String::as_str).unwrap_or("");

        match tag {
            0x00 => {
                depth += 1;
                if !in_common && key.eq_ignore_ascii_case("common") {
                    in_common = true;
                    common_depth = depth;
                    found_common = true;
                } else if in_common && depth == common_depth + 1 {
                    if key.eq_ignore_ascii_case("library_assets") {
                        flags.has_library_assets = true;
                    } else if key.eq_ignore_ascii_case("header_image") {
                        flags.has_header_image = true;
                    }
                }
            }
            0x01 => {
                let val_bytes =
                    read_cstring_bytes(blob, &mut i).ok_or(AppInfoError::Truncated {
                        context: "cstring value in indexed blob",
                    })?;
                if in_common && depth == common_depth {
                    if (key.eq_ignore_ascii_case("visibility")
                        || key.eq_ignore_ascii_case("section_type"))
                        && !val_bytes.is_empty()
                    {
                        flags.visibility =
                            Some(String::from_utf8_lossy(val_bytes).to_ascii_lowercase());
                    } else if key.eq_ignore_ascii_case("store_asset_mtime") {
                        flags.has_store_asset_mtime = true;
                    }
                }
            }
            0x02 | 0x03 | 0x04 | 0x06 => {
                if in_common
                    && depth == common_depth
                    && key.eq_ignore_ascii_case("store_asset_mtime")
                {
                    flags.has_store_asset_mtime = true;
                }
                if i + 4 > blob.len() {
                    return Err(AppInfoError::Truncated {
                        context: "4-byte value in indexed blob",
                    });
                }
                i += 4;
            }
            0x07 | 0x09 | 0x0a => {
                if in_common
                    && depth == common_depth
                    && key.eq_ignore_ascii_case("store_asset_mtime")
                {
                    flags.has_store_asset_mtime = true;
                }
                if i + 8 > blob.len() {
                    return Err(AppInfoError::Truncated {
                        context: "8-byte value in indexed blob",
                    });
                }
                i += 8;
            }
            _ => return Ok(if found_common { Some(flags) } else { None }),
        }
    }

    Ok(if found_common { Some(flags) } else { None })
}

fn read_cstring_bytes<'a>(blob: &'a [u8], pos: &mut usize) -> Option<&'a [u8]> {
    let start = *pos;
    let nul = blob[start..].iter().position(|&b| b == 0)?;
    *pos = start + nul + 1;
    Some(&blob[start..start + nul])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le32(v: u32) -> [u8; 4] {
        v.to_le_bytes()
    }

    fn le64(v: u64) -> [u8; 8] {
        v.to_le_bytes()
    }

    fn dummy_record_fixed_header() -> Vec<u8> {
        let mut h = Vec::new();
        h.extend_from_slice(&le32(1));
        h.extend_from_slice(&le32(1_700_000_000u32));
        h.extend_from_slice(&le64(0));
        h.extend_from_slice(&[0u8; 20]);
        h.extend_from_slice(&le32(42));
        h.extend_from_slice(&[0u8; 20]);
        h
    }

    fn v7_header() -> Vec<u8> {
        let mut h = vec![];
        h.extend_from_slice(&le32(MAGIC_CSTRING_V1));
        h.extend_from_slice(&le32(1));
        h
    }

    fn v8_header() -> Vec<u8> {
        let mut h = vec![];
        h.extend_from_slice(&le32(MAGIC_CSTRING_V2));
        h.extend_from_slice(&le32(1));
        h
    }

    fn build_cstring_blob_visibility(vis: &str) -> Vec<u8> {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"visibility\x00");
        blob.extend_from_slice(vis.as_bytes());
        blob.push(0x00);
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_cstring_blob_section_type(val: &str) -> Vec<u8> {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"section_type\x00");
        blob.extend_from_slice(val.as_bytes());
        blob.push(0x00);
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_cstring_blob_no_markers() -> Vec<u8> {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"name\x00");
        blob.extend_from_slice(b"Dota 2\x00");
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_cstring_blob_with_store_asset_mtime() -> Vec<u8> {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x02u8);
        blob.extend_from_slice(b"store_asset_mtime\x00");
        blob.extend_from_slice(&le32(1_700_000_000u32));
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_cstring_blob_with_library_assets() -> Vec<u8> {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_assets\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"library_capsule\x00");
        blob.extend_from_slice(b"some_hash\x00");
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_cstring_blob_with_header_image() -> Vec<u8> {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"header_image\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"english\x00");
        blob.extend_from_slice(b"header.jpg\x00");
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_cstring_blob_all_flags(vis: &str) -> Vec<u8> {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"visibility\x00");
        blob.extend_from_slice(vis.as_bytes());
        blob.push(0x00);
        blob.push(0x02u8);
        blob.extend_from_slice(b"store_asset_mtime\x00");
        blob.extend_from_slice(&le32(1_700_000_000u32));
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_assets\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"library_capsule\x00");
        blob.extend_from_slice(b"hash\x00");
        blob.push(0x08);
        blob.push(0x00u8);
        blob.extend_from_slice(b"header_image\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"english\x00");
        blob.extend_from_slice(b"header.jpg\x00");
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_v8_file(records: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let mut file = v8_header();
        for (app_id, blob) in records {
            let fixed_hdr = dummy_record_fixed_header();
            let body_size = fixed_hdr.len() + blob.len();
            file.extend_from_slice(&le32(*app_id));
            file.extend_from_slice(&le32(body_size as u32));
            file.extend_from_slice(&fixed_hdr);
            file.extend_from_slice(blob);
        }
        file.extend_from_slice(&le32(0));
        file
    }

    fn build_v9_file(records: &[(u32, Vec<u8>)], extra_strings: &[&str]) -> Vec<u8> {
        let mut file_body: Vec<u8> = Vec::new();

        let fixed_hdr = dummy_record_fixed_header();
        for (app_id, blob) in records {
            let body_size = fixed_hdr.len() + blob.len();
            file_body.extend_from_slice(&le32(*app_id));
            file_body.extend_from_slice(&le32(body_size as u32));
            file_body.extend_from_slice(&fixed_hdr);
            file_body.extend_from_slice(blob);
        }
        file_body.extend_from_slice(&le32(0));

        let st_offset = 16 + file_body.len();
        let mut string_table: Vec<u8> = Vec::new();
        let all_strings: Vec<&str> = ["appinfo", "appid", "common", "name", "type"]
            .iter()
            .chain(extra_strings.iter())
            .copied()
            .collect();
        string_table.extend_from_slice(&le32(all_strings.len() as u32));
        for s in &all_strings {
            string_table.extend_from_slice(s.as_bytes());
            string_table.push(0x00);
        }

        let mut file = Vec::new();
        file.extend_from_slice(&le32(MAGIC_STRING_TABLE));
        file.extend_from_slice(&le32(1));
        file.extend_from_slice(&le64(st_offset as u64));
        file.extend_from_slice(&file_body);
        file.extend_from_slice(&string_table);
        file
    }

    fn build_indexed_blob_with_section_type(
        common_idx: u32,
        section_type_idx: u32,
        val: &str,
    ) -> Vec<u8> {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(common_idx));
        blob.push(0x01u8);
        blob.extend_from_slice(&le32(section_type_idx));
        blob.extend_from_slice(val.as_bytes());
        blob.push(0x00);
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_indexed_blob_no_section_type(common_idx: u32, name_idx: u32) -> Vec<u8> {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(common_idx));
        blob.push(0x01u8);
        blob.extend_from_slice(&le32(name_idx));
        blob.extend_from_slice(b"Dota 2\x00");
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_indexed_blob_with_outer_wrapper(
        appinfo_idx: u32,
        appid_idx: u32,
        common_idx: u32,
        section_type_idx: u32,
        val: &str,
    ) -> Vec<u8> {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(appinfo_idx));
        blob.push(0x02u8);
        blob.extend_from_slice(&le32(appid_idx));
        blob.extend_from_slice(&le32(12345u32));
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(common_idx));
        blob.push(0x01u8);
        blob.extend_from_slice(&le32(section_type_idx));
        blob.extend_from_slice(val.as_bytes());
        blob.push(0x00);
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    #[test]
    fn unknown_magic_returns_unsupported_magic_error() {
        let mut bytes = vec![0xDEu8, 0xAD, 0xBE, 0xEF];
        bytes.extend_from_slice(&le32(1));
        bytes.extend_from_slice(&le64(0));
        bytes.extend_from_slice(&le32(0));
        let err = parse_appinfo_flags(&bytes).unwrap_err();
        assert!(matches!(
            err,
            AppInfoError::UnsupportedMagic { magic: 0xEFBEADDE }
        ));
    }

    #[test]
    fn truncated_header_returns_truncated_error() {
        let bytes = [0x27u8, 0x44, 0x56, 0x07];
        let err = parse_appinfo_flags(&bytes).unwrap_err();
        assert!(matches!(err, AppInfoError::Truncated { .. }));
    }

    #[test]
    fn v8_single_app_with_visibility_ownersonly() {
        let blob = build_cstring_blob_visibility("ownersonly");
        let file = build_v8_file(&[(12345, blob)]);
        let map = parse_appinfo_flags(&file).unwrap();
        let flags = map.get(&12345).unwrap();
        assert_eq!(flags.visibility.as_deref(), Some("ownersonly"));
        assert!(!flags.has_store_asset_mtime);
        assert!(!flags.has_library_assets);
        assert!(!flags.has_header_image);
    }

    #[test]
    fn v8_single_app_with_section_type_ownersonly() {
        let blob = build_cstring_blob_section_type("ownersonly");
        let file = build_v8_file(&[(67890, blob)]);
        let map = parse_appinfo_flags(&file).unwrap();
        let flags = map.get(&67890).unwrap();
        assert_eq!(flags.visibility.as_deref(), Some("ownersonly"));
    }

    #[test]
    fn v8_single_app_missing_common_not_in_map() {
        let mut blob = Vec::new();
        blob.push(0x01u8);
        blob.extend_from_slice(b"name\x00something\x00");
        blob.push(0x08);
        let file = build_v8_file(&[(111, blob)]);
        let map = parse_appinfo_flags(&file).unwrap();
        assert!(!map.contains_key(&111));
    }

    #[test]
    fn v8_single_app_missing_markers_in_common_yields_default_flags() {
        let blob = build_cstring_blob_no_markers();
        let file = build_v8_file(&[(570, blob)]);
        let map = parse_appinfo_flags(&file).unwrap();
        let flags = map.get(&570).unwrap();
        assert_eq!(flags.visibility, None);
        assert!(!flags.has_store_asset_mtime);
        assert!(!flags.has_library_assets);
        assert!(!flags.has_header_image);
    }

    #[test]
    fn v8_multiple_apps_mixed_visibility() {
        let blob_owners = build_cstring_blob_visibility("ownersonly");
        let blob_public = build_cstring_blob_visibility("public");
        let blob_none = build_cstring_blob_no_markers();
        let file = build_v8_file(&[(1001, blob_owners), (1002, blob_public), (1003, blob_none)]);
        let map = parse_appinfo_flags(&file).unwrap();
        assert_eq!(
            map.get(&1001).unwrap().visibility.as_deref(),
            Some("ownersonly")
        );
        assert_eq!(
            map.get(&1002).unwrap().visibility.as_deref(),
            Some("public")
        );
        assert_eq!(map.get(&1003).unwrap().visibility, None);
    }

    #[test]
    fn v8_visibility_value_is_lowercased() {
        let blob = build_cstring_blob_visibility("OwnersOnly");
        let file = build_v8_file(&[(9999, blob)]);
        let map = parse_appinfo_flags(&file).unwrap();
        assert_eq!(
            map.get(&9999).unwrap().visibility.as_deref(),
            Some("ownersonly")
        );
    }

    #[test]
    fn v8_truncated_header_only_returns_truncated() {
        let mut file = v8_header();
        file.extend_from_slice(&le32(999));
        let err = parse_appinfo_flags(&file).unwrap_err();
        assert!(matches!(err, AppInfoError::Truncated { .. }));
    }

    #[test]
    fn v8_truncated_app_record_body_returns_truncated() {
        let fixed_hdr = dummy_record_fixed_header();
        let blob = build_cstring_blob_visibility("ownersonly");
        let body_size = fixed_hdr.len() + blob.len();
        let mut file = v8_header();
        file.extend_from_slice(&le32(55555));
        file.extend_from_slice(&le32(body_size as u32));
        file.extend_from_slice(&fixed_hdr);
        file.extend_from_slice(&blob[..blob.len() / 2]);
        let err = parse_appinfo_flags(&file).unwrap_err();
        assert!(matches!(err, AppInfoError::Truncated { .. }));
    }

    #[test]
    fn v7_magic_accepted_and_parses_visibility() {
        let blob = build_cstring_blob_visibility("ownersonly");
        let fixed_hdr = {
            let mut h = Vec::new();
            h.extend_from_slice(&le32(1));
            h.extend_from_slice(&le32(0));
            h.extend_from_slice(&le64(0));
            h.extend_from_slice(&[0u8; 20]);
            h.extend_from_slice(&le32(7));
            h
        };
        let body_size = fixed_hdr.len() + blob.len();
        let mut file = v7_header();
        file.extend_from_slice(&le32(77777));
        file.extend_from_slice(&le32(body_size as u32));
        file.extend_from_slice(&fixed_hdr);
        file.extend_from_slice(&blob);
        file.extend_from_slice(&le32(0));
        let map = parse_appinfo_flags(&file).unwrap();
        assert_eq!(
            map.get(&77777).unwrap().visibility.as_deref(),
            Some("ownersonly")
        );
    }

    #[test]
    fn v9_magic_with_indexed_blob_section_type_ownersonly() {
        let extra_strings = ["section_type"];
        let common_idx = 2u32;
        let section_type_idx = 5u32;
        let blob = build_indexed_blob_with_section_type(common_idx, section_type_idx, "ownersonly");
        let file = build_v9_file(&[(770720, blob)], &extra_strings);
        let map = parse_appinfo_flags(&file).unwrap();
        assert_eq!(
            map.get(&770720).unwrap().visibility.as_deref(),
            Some("ownersonly")
        );
    }

    #[test]
    fn v9_magic_with_indexed_blob_no_section_type_yields_default_flags() {
        let extra_strings: &[&str] = &[];
        let common_idx = 2u32;
        let name_idx = 3u32;
        let blob = build_indexed_blob_no_section_type(common_idx, name_idx);
        let file = build_v9_file(&[(570, blob)], extra_strings);
        let map = parse_appinfo_flags(&file).unwrap();
        let flags = map.get(&570).unwrap();
        assert_eq!(flags.visibility, None);
        assert!(!flags.has_store_asset_mtime);
        assert!(!flags.has_library_assets);
        assert!(!flags.has_header_image);
    }

    #[test]
    fn v9_truncated_string_table_returns_truncated() {
        let mut file = Vec::new();
        file.extend_from_slice(&le32(MAGIC_STRING_TABLE));
        file.extend_from_slice(&le32(1));
        let st_offset = 16 + 8u64;
        file.extend_from_slice(&le64(st_offset));
        file.extend_from_slice(&le32(0));
        file.extend_from_slice(&le32(0));
        file.extend_from_slice(&le32(9999));
        let err = parse_appinfo_flags(&file).unwrap_err();
        assert!(matches!(err, AppInfoError::Truncated { .. }));
    }

    #[test]
    fn v9_outer_appinfo_wrapper_common_found_at_depth_2() {
        let all_strings = ["appinfo", "appid", "common", "name", "type", "section_type"];
        let appinfo_idx = 0u32;
        let appid_idx = 1u32;
        let common_idx = 2u32;
        let section_type_idx = 5u32;

        let blob = build_indexed_blob_with_outer_wrapper(
            appinfo_idx,
            appid_idx,
            common_idx,
            section_type_idx,
            "ownersonly",
        );

        let mut file_body: Vec<u8> = Vec::new();
        let fixed_hdr = dummy_record_fixed_header();
        let body_size = fixed_hdr.len() + blob.len();
        file_body.extend_from_slice(&le32(99001u32));
        file_body.extend_from_slice(&le32(body_size as u32));
        file_body.extend_from_slice(&fixed_hdr);
        file_body.extend_from_slice(&blob);
        file_body.extend_from_slice(&le32(0));

        let st_offset_val: u64 = (16 + file_body.len()) as u64;
        let mut string_table: Vec<u8> = Vec::new();
        string_table.extend_from_slice(&le32(all_strings.len() as u32));
        for s in &all_strings {
            string_table.extend_from_slice(s.as_bytes());
            string_table.push(0x00);
        }

        let mut file = Vec::new();
        file.extend_from_slice(&le32(MAGIC_STRING_TABLE));
        file.extend_from_slice(&le32(1));
        file.extend_from_slice(&le64(st_offset_val));
        file.extend_from_slice(&file_body);
        file.extend_from_slice(&string_table);

        let map = parse_appinfo_flags(&file).unwrap();
        assert_eq!(
            map.get(&99001).unwrap().visibility.as_deref(),
            Some("ownersonly")
        );
    }

    #[test]
    fn v8_non_utf8_latin1_string_value_does_not_return_error() {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"name\x00");
        blob.extend_from_slice(b"Moje Spore v\xfdtvory\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"section_type\x00");
        blob.extend_from_slice(b"ownersonly\x00");
        blob.push(0x08);
        blob.push(0x08);

        let file = build_v8_file(&[(17390, blob)]);
        let map = parse_appinfo_flags(&file).unwrap();
        assert_eq!(
            map.get(&17390).unwrap().visibility.as_deref(),
            Some("ownersonly")
        );
    }

    #[test]
    fn v9_int64_tag_0x0a_is_skipped_and_parsing_continues() {
        let extra_strings = ["section_type", "score"];
        let common_idx = 2u32;
        let section_type_idx = 5u32;
        let score_idx = 6u32;

        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(common_idx));
        blob.push(0x0au8);
        blob.extend_from_slice(&le32(score_idx));
        blob.extend_from_slice(&le64(9_999_999_999u64));
        blob.push(0x01u8);
        blob.extend_from_slice(&le32(section_type_idx));
        blob.extend_from_slice(b"ownersonly\x00");
        blob.push(0x08);
        blob.push(0x08);

        let file = build_v9_file(&[(55555, blob)], &extra_strings);
        let map = parse_appinfo_flags(&file).unwrap();
        assert_eq!(
            map.get(&55555).unwrap().visibility.as_deref(),
            Some("ownersonly")
        );
    }

    #[test]
    fn flag_only_store_asset_mtime() {
        let blob = build_cstring_blob_with_store_asset_mtime();
        let file = build_v8_file(&[(10001, blob)]);
        let map = parse_appinfo_flags(&file).unwrap();
        let flags = map.get(&10001).unwrap();
        assert_eq!(flags.visibility, None);
        assert!(flags.has_store_asset_mtime);
        assert!(!flags.has_library_assets);
        assert!(!flags.has_header_image);
    }

    #[test]
    fn flag_only_library_assets() {
        let blob = build_cstring_blob_with_library_assets();
        let file = build_v8_file(&[(10002, blob)]);
        let map = parse_appinfo_flags(&file).unwrap();
        let flags = map.get(&10002).unwrap();
        assert_eq!(flags.visibility, None);
        assert!(!flags.has_store_asset_mtime);
        assert!(flags.has_library_assets);
        assert!(!flags.has_header_image);
    }

    #[test]
    fn flag_only_header_image() {
        let blob = build_cstring_blob_with_header_image();
        let file = build_v8_file(&[(10003, blob)]);
        let map = parse_appinfo_flags(&file).unwrap();
        let flags = map.get(&10003).unwrap();
        assert_eq!(flags.visibility, None);
        assert!(!flags.has_store_asset_mtime);
        assert!(!flags.has_library_assets);
        assert!(flags.has_header_image);
    }

    #[test]
    fn flag_all_four_set() {
        let blob = build_cstring_blob_all_flags("ownersonly");
        let file = build_v8_file(&[(10004, blob)]);
        let map = parse_appinfo_flags(&file).unwrap();
        let flags = map.get(&10004).unwrap();
        assert_eq!(flags.visibility.as_deref(), Some("ownersonly"));
        assert!(flags.has_store_asset_mtime);
        assert!(flags.has_library_assets);
        assert!(flags.has_header_image);
    }

    #[test]
    fn flag_none_set_playtest_no_store_presence() {
        let blob = build_cstring_blob_no_markers();
        let file = build_v8_file(&[(10005, blob)]);
        let map = parse_appinfo_flags(&file).unwrap();
        let flags = map.get(&10005).unwrap();
        assert_eq!(flags.visibility, None);
        assert!(!flags.has_store_asset_mtime);
        assert!(!flags.has_library_assets);
        assert!(!flags.has_header_image);
    }

    fn fake_hash(n: u8) -> String {
        format!("{:040x}", n)
    }

    fn build_cstring_blob_with_laf_all_slots() -> Vec<u8> {
        let capsule = format!("{}/library_capsule.jpg\x00", fake_hash(0xAA));
        let hero = format!("{}/library_hero.jpg\x00", fake_hash(0xBB));
        let logo = format!("{}/logo.png\x00", fake_hash(0xCC));
        let header = format!("{}/library_header.jpg\x00", fake_hash(0xDD));

        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_assets_full\x00");
        for (slot, value) in [
            (&b"library_capsule"[..], capsule.as_bytes()),
            (b"library_hero", hero.as_bytes()),
            (b"library_logo", logo.as_bytes()),
            (b"library_header", header.as_bytes()),
        ] {
            blob.push(0x00u8);
            blob.extend_from_slice(slot);
            blob.push(0x00);
            blob.push(0x00u8);
            blob.extend_from_slice(b"image\x00");
            blob.push(0x01u8);
            blob.extend_from_slice(b"english\x00");
            blob.extend_from_slice(value);
            blob.push(0x08);
            blob.push(0x08);
        }
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_cstring_blob_with_laf_capsule_only() -> Vec<u8> {
        let capsule = format!("{}/library_capsule.jpg\x00", fake_hash(0x11));
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_assets_full\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_capsule\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"image\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"english\x00");
        blob.extend_from_slice(capsule.as_bytes());
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_indexed_blob_laf_all_slots(strings: &[&str]) -> Vec<u8> {
        let idx = |s: &str| -> u32 { strings.iter().position(|&x| x == s).unwrap() as u32 };
        let capsule_val = format!("{}/library_capsule.jpg\x00", fake_hash(0xAA));
        let hero_val = format!("{}/library_hero.jpg\x00", fake_hash(0xBB));
        let logo_val = format!("{}/logo.png\x00", fake_hash(0xCC));
        let header_val = format!("{}/library_header.jpg\x00", fake_hash(0xDD));

        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(idx("common")));
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(idx("library_assets_full")));
        for (slot, val) in [
            ("library_capsule", capsule_val.as_bytes()),
            ("library_hero", hero_val.as_bytes()),
            ("library_logo", logo_val.as_bytes()),
            ("library_header", header_val.as_bytes()),
        ] {
            blob.push(0x00u8);
            blob.extend_from_slice(&le32(idx(slot)));
            blob.push(0x00u8);
            blob.extend_from_slice(&le32(idx("image")));
            blob.push(0x01u8);
            blob.extend_from_slice(&le32(idx("english")));
            blob.extend_from_slice(val);
            blob.push(0x08);
            blob.push(0x08);
        }
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob
    }

    fn build_v9_assets_file(records: &[(u32, Vec<u8>)], extra_strings: &[&str]) -> Vec<u8> {
        let mut file_body: Vec<u8> = Vec::new();
        let fixed_hdr = dummy_record_fixed_header();
        for (app_id, blob) in records {
            let body_size = fixed_hdr.len() + blob.len();
            file_body.extend_from_slice(&le32(*app_id));
            file_body.extend_from_slice(&le32(body_size as u32));
            file_body.extend_from_slice(&fixed_hdr);
            file_body.extend_from_slice(blob);
        }
        file_body.extend_from_slice(&le32(0));

        let base_strings: &[&str] = &[
            "common",
            "library_assets_full",
            "library_capsule",
            "library_hero",
            "library_logo",
            "library_header",
            "image",
            "english",
        ];
        let all_strings: Vec<&str> = base_strings
            .iter()
            .chain(extra_strings.iter())
            .copied()
            .collect();

        let st_offset = 16 + file_body.len();
        let mut string_table: Vec<u8> = Vec::new();
        string_table.extend_from_slice(&le32(all_strings.len() as u32));
        for s in &all_strings {
            string_table.extend_from_slice(s.as_bytes());
            string_table.push(0x00);
        }

        let mut file = Vec::new();
        file.extend_from_slice(&le32(MAGIC_STRING_TABLE));
        file.extend_from_slice(&le32(1));
        file.extend_from_slice(&le64(st_offset as u64));
        file.extend_from_slice(&file_body);
        file.extend_from_slice(&string_table);
        file
    }

    fn hashed(hash_byte: u8, filename: &str) -> Option<ImageAsset> {
        Some(ImageAsset::Hashed {
            hash: fake_hash(hash_byte),
            filename: filename.to_owned(),
        })
    }

    fn plain(filename: &str) -> Option<ImageAsset> {
        Some(ImageAsset::Plain {
            filename: filename.to_owned(),
        })
    }

    #[test]
    fn assets_v8_all_four_slots_extracted() {
        let blob = build_cstring_blob_with_laf_all_slots();
        let file = build_v8_file(&[(20001, blob)]);
        let map = parse_appinfo_assets(&file).unwrap();
        let assets = map.get(&20001).unwrap();
        assert_eq!(assets.cover, hashed(0xAA, "library_capsule.jpg"));
        assert_eq!(assets.background, hashed(0xBB, "library_hero.jpg"));
        assert_eq!(assets.logo, hashed(0xCC, "logo.png"));
        assert_eq!(assets.wide_cover, hashed(0xDD, "library_header.jpg"));
    }

    #[test]
    fn assets_v8_capsule_only_other_slots_none() {
        let blob = build_cstring_blob_with_laf_capsule_only();
        let file = build_v8_file(&[(20002, blob)]);
        let map = parse_appinfo_assets(&file).unwrap();
        let assets = map.get(&20002).unwrap();
        assert_eq!(assets.cover, hashed(0x11, "library_capsule.jpg"));
        assert_eq!(assets.background, None);
        assert_eq!(assets.logo, None);
        assert_eq!(assets.wide_cover, None);
    }

    #[test]
    fn assets_v8_no_laf_section_not_in_map() {
        let blob = build_cstring_blob_no_markers();
        let file = build_v8_file(&[(20003, blob)]);
        let map = parse_appinfo_assets(&file).unwrap();
        assert!(!map.contains_key(&20003));
    }

    #[test]
    fn assets_v8_no_common_not_in_map() {
        let mut blob = Vec::new();
        blob.push(0x01u8);
        blob.extend_from_slice(b"name\x00something\x00");
        blob.push(0x08);
        let file = build_v8_file(&[(20004, blob)]);
        let map = parse_appinfo_assets(&file).unwrap();
        assert!(!map.contains_key(&20004));
    }

    #[test]
    fn assets_v8_plain_filename_yields_plain_asset() {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_assets_full\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_capsule\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"image\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"english\x00");
        blob.extend_from_slice(b"library_600x900.jpg\x00");
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        let file = build_v8_file(&[(20005, blob)]);
        let map = parse_appinfo_assets(&file).unwrap();
        let assets = map.get(&20005).unwrap();
        assert_eq!(assets.cover, plain("library_600x900.jpg"));
        assert_eq!(assets.background, None);
        assert_eq!(assets.logo, None);
        assert_eq!(assets.wide_cover, None);
    }

    #[test]
    fn assets_v8_plain_and_hashed_mixed_slots() {
        let hash_val = format!("{}/library_hero.jpg\x00", fake_hash(0xBB));
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_assets_full\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_capsule\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"image\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"english\x00");
        blob.extend_from_slice(b"library_600x900.jpg\x00");
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_hero\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"image\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"english\x00");
        blob.extend_from_slice(hash_val.as_bytes());
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        let file = build_v8_file(&[(20006, blob)]);
        let map = parse_appinfo_assets(&file).unwrap();
        let assets = map.get(&20006).unwrap();
        assert_eq!(assets.cover, plain("library_600x900.jpg"));
        assert_eq!(assets.background, hashed(0xBB, "library_hero.jpg"));
        assert_eq!(assets.logo, None);
        assert_eq!(assets.wide_cover, None);
    }

    #[test]
    fn assets_v9_all_four_slots_extracted() {
        let all_strings = [
            "common",
            "library_assets_full",
            "library_capsule",
            "library_hero",
            "library_logo",
            "library_header",
            "image",
            "english",
        ];
        let blob = build_indexed_blob_laf_all_slots(&all_strings);
        let file = build_v9_assets_file(&[(20010, blob)], &[]);
        let map = parse_appinfo_assets(&file).unwrap();
        let assets = map.get(&20010).unwrap();
        assert_eq!(assets.cover, hashed(0xAA, "library_capsule.jpg"));
        assert_eq!(assets.background, hashed(0xBB, "library_hero.jpg"));
        assert_eq!(assets.logo, hashed(0xCC, "logo.png"));
        assert_eq!(assets.wide_cover, hashed(0xDD, "library_header.jpg"));
    }

    #[test]
    fn assets_v9_no_laf_not_in_map() {
        let extra_strings: &[&str] = &["section_type"];
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(0u32)); // "common"
        blob.push(0x01u8);
        blob.extend_from_slice(&le32(8u32)); // "section_type"
        blob.extend_from_slice(b"game\x00");
        blob.push(0x08);
        blob.push(0x08);
        let file = build_v9_assets_file(&[(20011, blob)], extra_strings);
        let map = parse_appinfo_assets(&file).unwrap();
        assert!(!map.contains_key(&20011));
    }

    #[test]
    fn assets_v8_unknown_magic_returns_error() {
        let mut bytes = vec![0xDEu8, 0xAD, 0xBE, 0xEF];
        bytes.extend_from_slice(&le32(1));
        bytes.extend_from_slice(&le64(0));
        bytes.extend_from_slice(&le32(0));
        let err = parse_appinfo_assets(&bytes).unwrap_err();
        assert!(matches!(
            err,
            AppInfoError::UnsupportedMagic { magic: 0xEFBEADDE }
        ));
    }

    #[test]
    fn assets_v8_multiple_apps_only_laf_apps_in_map() {
        let blob_with_laf = build_cstring_blob_with_laf_capsule_only();
        let blob_no_laf = build_cstring_blob_no_markers();
        let file = build_v8_file(&[(30001, blob_with_laf), (30002, blob_no_laf)]);
        let map = parse_appinfo_assets(&file).unwrap();
        assert!(map.contains_key(&30001));
        assert!(!map.contains_key(&30002));
    }

    #[test]
    #[ignore = "requires real ~/.local/share/Steam/appcache/appinfo.vdf — run manually to verify app 2807960 hashes"]
    fn assets_real_appinfo_battlefield6() {
        let path =
            std::path::Path::new(env!("HOME")).join(".local/share/Steam/appcache/appinfo.vdf");
        let bytes = std::fs::read(&path).expect("appinfo.vdf not found");
        let map = parse_appinfo_assets(&bytes).expect("parse failed");
        let assets = map.get(&2807960).expect("app 2807960 not found in map");
        assert_eq!(
            assets.cover,
            Some(ImageAsset::Hashed {
                hash: "f94d928537ac1813f07baf86bbdc1b899fba7ddc".to_owned(),
                filename: "library_capsule.jpg".to_owned(),
            }),
        );
        assert_eq!(
            assets.logo,
            Some(ImageAsset::Hashed {
                hash: "3a5184881210f52887f766f92a313bd0902e7918".to_owned(),
                filename: "logo.png".to_owned(),
            }),
        );
        assert_eq!(
            assets.wide_cover,
            Some(ImageAsset::Hashed {
                hash: "1b277e018090ce15d169e22a6eca284338e3ce15".to_owned(),
                filename: "library_header.jpg".to_owned(),
            }),
        );
        let hero = assets.background.as_ref().expect("library_hero absent");
        let ImageAsset::Hashed { hash, .. } = hero else {
            panic!("library_hero should be Hashed for app 2807960");
        };
        assert_eq!(hash.len(), 40);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    #[ignore = "requires real ~/.local/share/Steam/appcache/appinfo.vdf — run manually to verify app 400 assets"]
    fn assets_real_appinfo_portal() {
        let path =
            std::path::Path::new(env!("HOME")).join(".local/share/Steam/appcache/appinfo.vdf");
        let bytes = std::fs::read(&path).expect("appinfo.vdf not found");
        let map = parse_appinfo_assets(&bytes).expect("parse failed");
        let assets = map.get(&400).expect("app 400 not found in map");
        let capsule = assets.cover.as_ref().expect("library_capsule absent");
        assert!(
            matches!(capsule, ImageAsset::Plain { filename } if filename == "library_600x900.jpg"),
            "expected Plain(library_600x900.jpg), got {capsule:?}",
        );
        let hero = assets.background.as_ref().expect("library_hero absent");
        assert!(
            matches!(hero, ImageAsset::Plain { filename } if filename == "library_hero.jpg"),
            "expected Plain(library_hero.jpg), got {hero:?}",
        );
        let header = assets.wide_cover.as_ref().expect("library_header absent");
        assert!(
            matches!(header, ImageAsset::Plain { filename } if filename == "header.jpg"),
            "expected Plain(header.jpg), got {header:?}",
        );
    }

    #[test]
    #[ignore = "requires real ~/.local/share/Steam/appcache/appinfo.vdf"]
    fn assets_real_appinfo_coverage_at_least_750() {
        let path =
            std::path::Path::new(env!("HOME")).join(".local/share/Steam/appcache/appinfo.vdf");
        let bytes = std::fs::read(&path).expect("appinfo.vdf not found");
        let map = parse_appinfo_assets(&bytes).expect("parse failed");
        let with_header = map.values().filter(|a| a.wide_cover.is_some()).count();
        assert!(
            with_header >= 750,
            "expected >=750 apps with library_header, got {with_header}"
        );
    }

    #[test]
    #[ignore = "requires real ~/.local/share/Steam/appcache/appinfo.vdf — verify app 249650 header_image fallback"]
    fn assets_real_appinfo_app_249650_has_library_header() {
        let path =
            std::path::Path::new(env!("HOME")).join(".local/share/Steam/appcache/appinfo.vdf");
        let bytes = std::fs::read(&path).expect("appinfo.vdf not found");
        let map = parse_appinfo_assets(&bytes).expect("parse failed");
        let assets = map.get(&249650).expect("app 249650 not in map");
        assert!(
            matches!(&assets.wide_cover, Some(ImageAsset::Plain { filename }) if filename == "header.jpg"),
            "expected Plain(header.jpg), got {:?}",
            assets.wide_cover
        );
        assert!(
            matches!(&assets.cover, Some(ImageAsset::Plain { filename }) if filename == "library_600x900.jpg"),
            "expected Plain(library_600x900.jpg), got {:?}",
            assets.cover
        );
    }

    #[test]
    fn assets_header_image_fallback_when_no_laf_header_slot() {
        // common {
        //   library_assets_full {
        //     library_capsule { image { "english" "library_600x900.jpg" } }
        //   }
        //   header_image { "english" "header.jpg" }
        // }
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_assets_full\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_capsule\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"image\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"english\x00");
        blob.extend_from_slice(b"library_600x900.jpg\x00");
        blob.push(0x08); // close image
        blob.push(0x08); // close library_capsule
        blob.push(0x08); // close library_assets_full
        blob.push(0x00u8);
        blob.extend_from_slice(b"header_image\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"english\x00");
        blob.extend_from_slice(b"header.jpg\x00");
        blob.push(0x08); // close header_image
        blob.push(0x08); // close common
        let file = build_v8_file(&[(400, blob)]);
        let map = parse_appinfo_assets(&file).unwrap();
        let assets = map.get(&400).unwrap();
        assert_eq!(assets.cover, plain("library_600x900.jpg"));
        assert_eq!(assets.background, None);
        assert_eq!(assets.logo, None);
        assert_eq!(assets.wide_cover, None);
        assert_eq!(
            assets.wide_cover_legacy,
            plain("header.jpg"),
            "common/header_image must populate header_image_legacy"
        );
    }

    #[test]
    fn assets_header_image_fallback_skipped_when_laf_header_slot_present() {
        // common {
        //   library_assets_full {
        //     library_header { image { "english" "{hash}/library_header.jpg" } }
        //   }
        //   header_image { "english" "header.jpg" }
        // }
        let header_val = format!("{}/library_header.jpg\x00", fake_hash(0xDD));
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_assets_full\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"library_header\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"image\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"english\x00");
        blob.extend_from_slice(header_val.as_bytes());
        blob.push(0x08); // close image
        blob.push(0x08); // close library_header
        blob.push(0x08); // close library_assets_full
        blob.push(0x00u8);
        blob.extend_from_slice(b"header_image\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"english\x00");
        blob.extend_from_slice(b"header.jpg\x00");
        blob.push(0x08); // close header_image
        blob.push(0x08); // close common
        let file = build_v8_file(&[(9001, blob)]);
        let map = parse_appinfo_assets(&file).unwrap();
        let assets = map.get(&9001).unwrap();
        assert_eq!(
            assets.wide_cover,
            hashed(0xDD, "library_header.jpg"),
            "LAF library_header must take priority over header_image fallback"
        );
    }

    #[test]
    fn assets_header_image_only_no_laf_yields_entry_with_header() {
        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(b"common\x00");
        blob.push(0x00u8);
        blob.extend_from_slice(b"header_image\x00");
        blob.push(0x01u8);
        blob.extend_from_slice(b"english\x00");
        blob.extend_from_slice(b"header.jpg\x00");
        blob.push(0x08);
        blob.push(0x08);
        blob.push(0x08);
        let file = build_v8_file(&[(215, blob)]);
        let map = parse_appinfo_assets(&file).unwrap();
        let assets = map
            .get(&215)
            .expect("app 215 must be in map via header_image");
        assert_eq!(assets.cover, None);
        assert_eq!(assets.background, None);
        assert_eq!(assets.logo, None);
        assert_eq!(assets.wide_cover, None);
        assert_eq!(assets.wide_cover_legacy, plain("header.jpg"));
    }

    #[test]
    fn assets_v9_header_image_fallback() {
        // common {
        //   library_assets_full {
        //     library_capsule { image { "english" "library_600x900.jpg" } }
        //   }
        //   header_image { "english" "header.jpg" }
        // }
        let all_strings = [
            "common",
            "library_assets_full",
            "library_capsule",
            "library_hero",
            "library_logo",
            "library_header",
            "image",
            "english",
            "header_image",
        ];
        let idx = |s: &str| -> u32 { all_strings.iter().position(|&x| x == s).unwrap() as u32 };

        let mut blob = Vec::new();
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(idx("common")));
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(idx("library_assets_full")));
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(idx("library_capsule")));
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(idx("image")));
        blob.push(0x01u8);
        blob.extend_from_slice(&le32(idx("english")));
        blob.extend_from_slice(b"library_600x900.jpg\x00");
        blob.push(0x08); // close image
        blob.push(0x08); // close library_capsule
        blob.push(0x08); // close library_assets_full
        blob.push(0x00u8);
        blob.extend_from_slice(&le32(idx("header_image")));
        blob.push(0x01u8);
        blob.extend_from_slice(&le32(idx("english")));
        blob.extend_from_slice(b"header.jpg\x00");
        blob.push(0x08); // close header_image
        blob.push(0x08); // close common

        let file = build_v9_assets_file(&[(400, blob)], &["header_image"]);
        let map = parse_appinfo_assets(&file).unwrap();
        let assets = map.get(&400).unwrap();
        assert_eq!(assets.cover, plain("library_600x900.jpg"));
        assert_eq!(assets.wide_cover, None);
        assert_eq!(assets.wide_cover_legacy, plain("header.jpg"));
    }
}
