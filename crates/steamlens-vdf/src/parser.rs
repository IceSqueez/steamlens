/// A key-value pair within a binary KeyValue section.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyValuePair {
    pub key: String,
    pub value: Value,
}

/// A typed value read from a binary KeyValue stream.
///
/// `WideString` (type tag 0x05 in the on-disk format) is deliberately absent.
/// Encountering it produces [`VdfError::UnsupportedType`] rather than a panic
/// or a silent skip, so callers can decide how to handle it.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Section(Vec<KeyValuePair>),
    String(std::string::String),
    Int32(i32),
    UInt64(u64),
    Float32(f32),
}

impl Value {
    pub fn as_section(&self) -> Option<&[KeyValuePair]> {
        match self {
            Value::Section(v) => Some(v.as_slice()),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Value::Int32(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::UInt64(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Value::Float32(f) => Some(*f),
            _ => None,
        }
    }

    /// Walk a `/`-separated path through nested sections and return the first
    /// matching [`Value`], or `None` if any segment is absent.
    ///
    /// Comparison is case-sensitive and matches the first child whose key
    /// equals the segment (Steam schema keys are lowercase ASCII in practice,
    /// but the format imposes no constraint).
    pub fn get(&self, path: &str) -> Option<&Value> {
        let mut current = self;
        for segment in path.split('/') {
            let pairs = current.as_section()?;
            current = pairs.iter().find(|p| p.key == segment).map(|p| &p.value)?;
        }
        Some(current)
    }
}

/// Errors produced by the binary KeyValue parser.
#[derive(Debug, thiserror::Error)]
pub enum VdfError {
    #[error("unexpected end of input at byte offset {offset}")]
    UnexpectedEof { offset: usize },

    #[error("unknown type tag {tag:#04x} at byte offset {offset}")]
    UnknownTypeTag { tag: u8, offset: usize },

    #[error("unsupported type {tag:#04x} (wstring) at byte offset {offset}")]
    UnsupportedType { tag: u8, offset: usize },

    #[error("invalid UTF-8 in key or string at byte offset {offset}: {source}")]
    InvalidUtf8 {
        offset: usize,
        source: std::string::FromUtf8Error,
    },
}

// ---------------------------------------------------------------------------
// Binary reader
// ---------------------------------------------------------------------------

pub(crate) struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Cursor { data, pos: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, VdfError> {
        match self.data.get(self.pos) {
            Some(&b) => {
                self.pos += 1;
                Ok(b)
            }
            None => Err(VdfError::UnexpectedEof { offset: self.pos }),
        }
    }

    fn read_exact(&mut self, n: usize) -> Result<&'a [u8], VdfError> {
        let start = self.pos;
        let end = start
            .checked_add(n)
            .ok_or(VdfError::UnexpectedEof { offset: start })?;
        match self.data.get(start..end) {
            Some(slice) => {
                self.pos = end;
                Ok(slice)
            }
            None => Err(VdfError::UnexpectedEof { offset: start }),
        }
    }

    fn read_null_terminated(&mut self) -> Result<String, VdfError> {
        let start = self.pos;
        loop {
            let b = self.read_u8()?;
            if b == 0x00 {
                break;
            }
        }
        // self.pos now points one past the null byte; the string is [start..self.pos-1]
        let raw = self.data[start..self.pos - 1].to_vec();
        String::from_utf8(raw).map_err(|source| VdfError::InvalidUtf8 {
            offset: start,
            source,
        })
    }

    fn read_i32(&mut self) -> Result<i32, VdfError> {
        let bytes = self.read_exact(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u32(&mut self) -> Result<u32, VdfError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, VdfError> {
        let bytes = self.read_exact(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_f32(&mut self) -> Result<f32, VdfError> {
        let bytes = self.read_exact(4)?;
        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub(crate) fn read_section(&mut self) -> Result<Value, VdfError> {
        let mut children = Vec::new();

        loop {
            let tag_offset = self.pos;
            let tag = self.read_u8()?;

            // Tag 0x08 = End — section is complete.
            if tag == 0x08 {
                return Ok(Value::Section(children));
            }

            let key = self.read_null_terminated()?;

            let value = match tag {
                // None / Section (tag 0x00) — recurse
                0x00 => self.read_section()?,

                // String (tag 0x01) — null-terminated UTF-8
                0x01 => Value::String(self.read_null_terminated()?),

                // Int32 (tag 0x02) — 4-byte little-endian signed
                0x02 => Value::Int32(self.read_i32()?),

                // Float32 (tag 0x03) — 4-byte little-endian IEEE 754
                0x03 => Value::Float32(self.read_f32()?),

                // Pointer (tag 0x04) — 4-byte little-endian u32; stored as UInt64
                // for lossless representation (same value, wider container).
                0x04 => Value::UInt64(u64::from(self.read_u32()?)),

                // WideString (tag 0x05) — not supported; the reference throws on this tag.
                0x05 => {
                    return Err(VdfError::UnsupportedType {
                        tag: 0x05,
                        offset: tag_offset,
                    });
                }

                // Color (tag 0x06) — 4-byte little-endian RGBA u32; stored as UInt64
                // for lossless representation.
                0x06 => Value::UInt64(u64::from(self.read_u32()?)),

                // UInt64 (tag 0x07) — 8-byte little-endian unsigned
                0x07 => Value::UInt64(self.read_u64()?),

                other => {
                    return Err(VdfError::UnknownTypeTag {
                        tag: other,
                        offset: tag_offset,
                    });
                }
            };

            children.push(KeyValuePair { key, value });
        }
    }
}
