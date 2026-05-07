/// `payload` is owned — Steam's internal buffer is freed before this
/// value becomes visible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCallback {
    pub id: i32,
    pub payload: Vec<u8>,
}

