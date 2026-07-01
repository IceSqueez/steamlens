#[derive(Debug, Clone, PartialEq)]
pub struct KeyValuePair {
    pub key: String,
    pub value: Value,
}

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

    pub fn get(&self, path: &str) -> Option<&Value> {
        let mut current = self;
        for segment in path.split('/') {
            let pairs = current.as_section()?;
            current = pairs.iter().find(|p| p.key == segment).map(|p| &p.value)?;
        }
        Some(current)
    }
}

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

    #[error("maximum nesting depth exceeded at byte offset {offset}")]
    MaxDepthExceeded { offset: usize },
}

const TAG_SECTION: u8 = 0x00;
const TAG_STRING: u8 = 0x01;
const TAG_INT32: u8 = 0x02;
const TAG_FLOAT32: u8 = 0x03;
const TAG_PTR: u8 = 0x04;
const TAG_WSTRING: u8 = 0x05;
const TAG_COLOR: u8 = 0x06;
const TAG_UINT64: u8 = 0x07;
const TAG_SECTION_END: u8 = 0x08;

pub(crate) const MAX_SECTION_DEPTH: usize = 128;

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
        let raw = self.data[start..self.pos - 1].to_vec();
        String::from_utf8(raw).map_err(|source| VdfError::InvalidUtf8 {
            offset: start,
            source,
        })
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], VdfError> {
        let slice = self.read_exact(N)?;
        let mut arr = [0u8; N];
        arr.copy_from_slice(slice);
        Ok(arr)
    }

    pub(crate) fn read_section(&mut self) -> Result<Value, VdfError> {
        self.read_section_at_depth(1)
    }

    fn read_section_at_depth(&mut self, depth: usize) -> Result<Value, VdfError> {
        if depth > MAX_SECTION_DEPTH {
            return Err(VdfError::MaxDepthExceeded { offset: self.pos });
        }

        let mut children = Vec::new();

        loop {
            let tag_offset = self.pos;
            let tag = self.read_u8()?;

            if tag == TAG_SECTION_END {
                return Ok(Value::Section(children));
            }

            let key = self.read_null_terminated()?;

            let value = match tag {
                TAG_SECTION => self.read_section_at_depth(depth + 1)?,
                TAG_STRING => Value::String(self.read_null_terminated()?),
                TAG_INT32 => Value::Int32(i32::from_le_bytes(self.read_array()?)),
                TAG_FLOAT32 => Value::Float32(f32::from_le_bytes(self.read_array()?)),
                TAG_PTR => Value::UInt64(u64::from(u32::from_le_bytes(self.read_array()?))),
                TAG_WSTRING => {
                    return Err(VdfError::UnsupportedType {
                        tag: TAG_WSTRING,
                        offset: tag_offset,
                    });
                }
                TAG_COLOR => Value::UInt64(u64::from(u32::from_le_bytes(self.read_array()?))),
                TAG_UINT64 => Value::UInt64(u64::from_le_bytes(self.read_array()?)),
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
