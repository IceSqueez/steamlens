#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCallback {
    pub id: i32,
    pub payload: Vec<u8>,
}
