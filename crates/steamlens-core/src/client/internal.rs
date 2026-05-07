pub(super) fn decode_steam_buf(buf: &[u8], written: usize) -> Option<String> {
    let len = written.min(buf.len());
    let trimmed = buf[..len]
        .iter()
        .position(|&b| b == 0)
        .map_or(&buf[..len], |nul| &buf[..nul]);
    if trimmed.is_empty() {
        return None;
    }
    String::from_utf8(trimmed.to_vec()).ok()
}

pub(super) fn nul_terminated_str(buf: &[u8]) -> Option<&str> {
    let nul_pos = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    if nul_pos == 0 {
        return None;
    }
    std::str::from_utf8(&buf[..nul_pos]).ok()
}
