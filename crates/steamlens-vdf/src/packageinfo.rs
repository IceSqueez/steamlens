use std::collections::BTreeMap;

use thiserror::Error;

use crate::parser::{Value, VdfError};

const PACKAGEINFO_MAGIC: u32 = 0x0656_5528;

/// Errors produced by the `packageinfo.vdf` binary parser.
#[derive(Debug, Error)]
pub enum PackageInfoError {
    /// The first 4 bytes of the file did not match the expected magic number.
    #[error(
        "packageinfo.vdf magic mismatch: expected 0x{:08x}, got 0x{magic:08x}",
        PACKAGEINFO_MAGIC
    )]
    MalformedHeader { magic: u32 },

    /// The file ended before a complete record could be read.
    #[error("packageinfo.vdf is truncated")]
    Truncated,

    /// A binary KV blob inside a package record failed to parse.
    #[error("packageinfo.vdf inner KV parse error: {0}")]
    InnerKvParse(#[source] VdfError),

    /// A package record contained no `appids` child section.
    ///
    /// This variant is currently used for packages that parse correctly but
    /// contain no `appids` key; those are silently skipped during enumeration.
    /// Kept for completeness and future diagnostics.
    #[error("packageinfo.vdf record is missing the appids block")]
    MissingAppidsBlock,
}

/// Parse the binary `packageinfo.vdf` wrapper and return a sorted
/// `(app_id → change_number)` map deduplicating apps that appear in
/// multiple packages.
///
/// `change_number` is the package's PICS change number — it advances when
/// any depot/manifest of the package is updated. When an app appears in
/// multiple packages we keep the maximum `change_number` (most recent
/// observed change). Callers can use this number as a coarse "has this
/// app's package been touched since I last looked?" signal for cache
/// invalidation; it over-invalidates by package boundary (any sibling app
/// update bumps it) but never under-invalidates.
///
/// The slice must contain the full file contents, starting with the 8-byte
/// header. Truncated or corrupted records error out — partial results are
/// not returned.
pub fn parse_packageinfo(bytes: &[u8]) -> Result<Vec<(u32, u32)>, PackageInfoError> {
    if bytes.len() < 8 {
        return Err(PackageInfoError::Truncated);
    }

    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic != PACKAGEINFO_MAGIC {
        return Err(PackageInfoError::MalformedHeader { magic });
    }

    let mut cursor = 8usize;
    let mut app_ids: BTreeMap<u32, u32> = BTreeMap::new();

    loop {
        if cursor + 4 > bytes.len() {
            return Err(PackageInfoError::Truncated);
        }

        let package_id = u32::from_le_bytes([
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            bytes[cursor + 3],
        ]);
        cursor += 4;

        if package_id == 0xFFFF_FFFF {
            break;
        }

        // sha1 (20 bytes) + change_number (4 bytes) + pics_token (8 bytes) = 32 bytes
        if cursor + 32 > bytes.len() {
            return Err(PackageInfoError::Truncated);
        }
        // Skip sha1 (20 bytes), then read change_number (4 bytes), then skip pics_token (8 bytes)
        let change_number = u32::from_le_bytes([
            bytes[cursor + 20],
            bytes[cursor + 21],
            bytes[cursor + 22],
            bytes[cursor + 23],
        ]);
        cursor += 32;

        let blob_slice = &bytes[cursor..];
        let blob_len = match scan_kv_blob_length(blob_slice) {
            Some(n) => n,
            None => return Err(PackageInfoError::Truncated),
        };

        let blob = &bytes[cursor..cursor + blob_len];
        cursor += blob_len;

        let root = match crate::parse(blob) {
            Ok(v) => v,
            Err(e) => return Err(PackageInfoError::InnerKvParse(e)),
        };

        let inner = match root
            .as_section()
            .and_then(|children| children.iter().next().and_then(|p| p.value.as_section()))
        {
            Some(s) => s,
            None => continue,
        };

        let appids_block = inner
            .iter()
            .find(|p| p.key == "appids")
            .and_then(|p| p.value.as_section());

        let Some(appids_children) = appids_block else {
            continue;
        };

        for child in appids_children {
            let id = match &child.value {
                Value::Int32(v) => *v as u32,
                Value::UInt64(v) => *v as u32,
                _ => continue,
            };
            app_ids
                .entry(id)
                .and_modify(|cn| {
                    if change_number > *cn {
                        *cn = change_number;
                    }
                })
                .or_insert(change_number);
        }
    }

    Ok(app_ids.into_iter().collect())
}

/// Walk a binary KV stream starting at `bytes[0]` and return the byte count
/// consumed by the outermost section (including its closing `0x08` marker).
///
/// Returns `None` when the stream is malformed or truncated before the
/// matching close marker is found.
pub(crate) fn scan_kv_blob_length(bytes: &[u8]) -> Option<usize> {
    let mut i = 0usize;
    let mut depth: i32 = 0;
    while i < bytes.len() {
        let tag = bytes[i];
        i += 1;
        match tag {
            0x00 => {
                let end = find_cstr_end(&bytes[i..])?;
                i += end + 1;
                depth += 1;
            }
            0x01 => {
                let end_k = find_cstr_end(&bytes[i..])?;
                i += end_k + 1;
                let end_v = find_cstr_end(&bytes[i..])?;
                i += end_v + 1;
            }
            0x02 | 0x03 | 0x04 | 0x06 => {
                let end_k = find_cstr_end(&bytes[i..])?;
                i = i.checked_add(end_k + 1 + 4)?;
                if i > bytes.len() {
                    return None;
                }
            }
            0x07 => {
                let end_k = find_cstr_end(&bytes[i..])?;
                i = i.checked_add(end_k + 1 + 8)?;
                if i > bytes.len() {
                    return None;
                }
            }
            0x08 => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => return None,
        }
    }
    None
}

fn find_cstr_end(bytes: &[u8]) -> Option<usize> {
    bytes.iter().position(|&b| b == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn magic_bytes() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&PACKAGEINFO_MAGIC.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes()); // universe = 1
        v
    }

    fn terminator() -> Vec<u8> {
        0xFFFF_FFFFu32.to_le_bytes().to_vec()
    }

    fn package_record(package_id: u32, change_number: u32, app_ids: &[u32]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&package_id.to_le_bytes());
        v.extend_from_slice(&[0u8; 20]); // sha1
        v.extend_from_slice(&change_number.to_le_bytes());
        v.extend_from_slice(&[0u8; 8]); // pics_token

        let blob = build_package_blob(package_id, app_ids);
        v.extend_from_slice(&blob);
        v
    }

    fn build_package_blob(package_id: u32, app_ids: &[u32]) -> Vec<u8> {
        let mut appids_children = Vec::new();
        for (i, &id) in app_ids.iter().enumerate() {
            // Each app_id stored as Int32, key is stringified index
            let key = i.to_string();
            appids_children.push(0x02u8); // Int32
            appids_children.extend_from_slice(key.as_bytes());
            appids_children.push(0x00);
            appids_children.extend_from_slice(&(id as i32).to_le_bytes());
        }
        appids_children.push(0x08); // end appids section

        let mut inner = Vec::new();
        inner.push(0x00u8); // Section tag
        inner.extend_from_slice(b"appids");
        inner.push(0x00);
        inner.extend_from_slice(&appids_children);
        inner.push(0x08); // end inner section

        let pkg_key = package_id.to_string();
        let mut blob = Vec::new();
        blob.push(0x00u8); // Section tag for outer key = package_id string
        blob.extend_from_slice(pkg_key.as_bytes());
        blob.push(0x00);
        blob.extend_from_slice(&inner);
        blob.push(0x08); // end outer blob
        blob
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&terminator());
        let err = parse_packageinfo(&bytes).unwrap_err();
        assert!(matches!(err, PackageInfoError::MalformedHeader { .. }));
    }

    #[test]
    fn truncated_header_returns_error() {
        let bytes = [0x28u8, 0x55, 0x56]; // only 3 bytes — too short
        let err = parse_packageinfo(&bytes).unwrap_err();
        assert!(matches!(err, PackageInfoError::Truncated));
    }

    #[test]
    fn terminator_only_returns_empty_vec() {
        let mut bytes = magic_bytes();
        bytes.extend_from_slice(&terminator());
        let ids = parse_packageinfo(&bytes).unwrap();
        assert!(ids.is_empty());
    }

    #[test]
    fn single_package_two_app_ids_returned_sorted() {
        let mut bytes = magic_bytes();
        bytes.extend_from_slice(&package_record(42, 7777, &[12345, 67890]));
        bytes.extend_from_slice(&terminator());

        let ids = parse_packageinfo(&bytes).unwrap();
        assert_eq!(ids, vec![(12345, 7777), (67890, 7777)]);
    }

    #[test]
    fn deduplication_across_packages_keeps_max_change_number() {
        let mut bytes = magic_bytes();
        bytes.extend_from_slice(&package_record(1, 100, &[100, 200]));
        bytes.extend_from_slice(&package_record(2, 200, &[200, 300]));
        bytes.extend_from_slice(&terminator());

        let ids = parse_packageinfo(&bytes).unwrap();
        // app_id 200 is in both packages; we keep max(100, 200) = 200
        assert_eq!(ids, vec![(100, 100), (200, 200), (300, 200)]);
    }

    #[test]
    fn package_without_appids_block_is_silently_skipped() {
        let mut bytes = magic_bytes();
        // A package with no appids section — just an empty inner blob
        let mut record = Vec::new();
        record.extend_from_slice(&99u32.to_le_bytes()); // package_id
        record.extend_from_slice(&[0u8; 32]); // sha1 + change_number + pics_token
        // blob: outer section with one child (no appids)
        let blob = {
            let mut b = Vec::new();
            b.push(0x00u8); // Section
            b.extend_from_slice(b"99");
            b.push(0x00);
            b.push(0x08); // empty inner section
            b.push(0x08); // end outer
            b
        };
        record.extend_from_slice(&blob);
        bytes.extend_from_slice(&record);
        bytes.extend_from_slice(&terminator());

        let ids = parse_packageinfo(&bytes).unwrap();
        assert!(ids.is_empty());
    }

    #[test]
    fn truncated_mid_record_returns_truncated_error() {
        let mut bytes = magic_bytes();
        bytes.extend_from_slice(&100u32.to_le_bytes()); // package_id
        // deliberately stop here — no sha1, no blob
        let err = parse_packageinfo(&bytes).unwrap_err();
        assert!(matches!(err, PackageInfoError::Truncated));
    }
}
