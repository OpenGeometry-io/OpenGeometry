mod ring;
#[cfg(test)]
mod tests;

pub(crate) use ring::{offset_ring_variable, OffsetError, OffsetHole};
