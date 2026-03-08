use std::io::Read;

mod evt;
pub mod header;

pub struct RawReader<T: Read> {
    inner: T,
}

impl<T: Read> RawReader<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}
