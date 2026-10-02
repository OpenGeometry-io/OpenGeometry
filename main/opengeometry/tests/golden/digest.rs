use sha2::{Digest, Sha256};

pub(crate) trait LittleEndianWord: Copy {
    type Bytes: AsRef<[u8]>;

    fn little_endian_bytes(self) -> Self::Bytes;
}

impl LittleEndianWord for f64 {
    type Bytes = [u8; 8];

    fn little_endian_bytes(self) -> Self::Bytes {
        self.to_le_bytes()
    }
}

impl LittleEndianWord for f32 {
    type Bytes = [u8; 4];

    fn little_endian_bytes(self) -> Self::Bytes {
        self.to_le_bytes()
    }
}

impl LittleEndianWord for u32 {
    type Bytes = [u8; 4];

    fn little_endian_bytes(self) -> Self::Bytes {
        self.to_le_bytes()
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

pub(crate) fn sha256_words<Word: LittleEndianWord>(values: &[Word]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.little_endian_bytes());
    }
    hex(&hasher.finalize())
}

pub(crate) fn bits(value: f64) -> String {
    format!("{:016x} {value:?}", value.to_bits())
}

pub(crate) fn bits_list(values: &[f64]) -> String {
    values
        .iter()
        .map(|value| format!("{:016x}", value.to_bits()))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn triples(values: &[f64]) -> String {
    let mut text = String::with_capacity(values.len() * 17);
    for triple in values.chunks(3) {
        text.push_str(&bits_list(triple));
        text.push('\n');
    }
    text
}
