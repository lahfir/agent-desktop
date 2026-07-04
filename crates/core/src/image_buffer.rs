#[derive(Debug)]
pub struct ImageBuffer {
    pub data: Vec<u8>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug)]
pub enum ImageFormat {
    Png,
    Jpg,
}

impl ImageFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpg => "jpg",
        }
    }
}

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
const PNG_IHDR_CHUNK_TYPE: &[u8; 4] = b"IHDR";

/// Reads width/height from a PNG's `IHDR` chunk, validating the 8-byte PNG
/// signature and the `IHDR` chunk type before trusting the byte offsets.
/// Returns `None` for anything that is not a well-formed PNG header,
/// including truncated buffers.
pub fn parse_png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 24 {
        return None;
    }
    if data[0..8] != PNG_SIGNATURE {
        return None;
    }
    if &data[12..16] != PNG_IHDR_CHUNK_TYPE {
        return None;
    }
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.extend_from_slice(&[0, 0, 0, 13]);
        bytes.extend_from_slice(PNG_IHDR_CHUNK_TYPE);
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    #[test]
    fn parses_dimensions_from_valid_png_header() {
        let bytes = valid_png_header(640, 480);

        assert_eq!(parse_png_dimensions(&bytes), Some((640, 480)));
    }

    #[test]
    fn rejects_undersized_buffer() {
        assert_eq!(parse_png_dimensions(&[1, 2, 3]), None);
    }

    #[test]
    fn rejects_buffer_with_wrong_signature() {
        let mut bytes = valid_png_header(640, 480);
        bytes[0] = 0x00;

        assert_eq!(parse_png_dimensions(&bytes), None);
    }

    #[test]
    fn rejects_buffer_with_wrong_chunk_type() {
        let mut bytes = valid_png_header(640, 480);
        bytes[12..16].copy_from_slice(b"IDAT");

        assert_eq!(parse_png_dimensions(&bytes), None);
    }
}
