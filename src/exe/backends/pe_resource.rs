use std::path::Path;

use image::imageops::FilterType;

use crate::exe::error::ExeThumbError;
use crate::exe::extractor::ExeIconExtractor;

const ICO_HEADER_LEN: usize = 6;
const ICO_DIR_ENTRY_LEN: usize = 16;
const MAX_ICON_DIR_ENTRIES: usize = 64;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PNG_IEND: &[u8; 4] = b"IEND";

pub struct PeResourceIconExtractor;

impl ExeIconExtractor for PeResourceIconExtractor {
    fn extract_best_icon(&self, path: &Path, out: &Path, size: u32) -> Result<(), ExeThumbError> {
        extract_ico_blob(path, out, size)
    }

    fn backend_name(&self) -> &'static str {
        "pe-resource"
    }
}

fn extract_ico_blob(path: &Path, out: &Path, size: u32) -> Result<(), ExeThumbError> {
    let bytes = std::fs::read(path).map_err(|source| ExeThumbError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let decoded = find_best_png_icon(&bytes, size)
        .or_else(|| find_ico_blob(&bytes).and_then(decode_icon_blob))
        .ok_or_else(|| ExeThumbError::NoIconResource {
            path: path.to_path_buf(),
        })?;

    let target = decoded.resize(size, size, FilterType::CatmullRom);

    target
        .save(out)
        .map_err(|source| ExeThumbError::DecodeFailed {
            path: out.to_path_buf(),
            reason: source.to_string(),
        })
}

fn decode_icon_blob(bytes: &[u8]) -> Option<image::DynamicImage> {
    image::load_from_memory(bytes).ok()
}

fn find_best_png_icon(bytes: &[u8], size: u32) -> Option<image::DynamicImage> {
    let mut best_match: Option<(u64, image::DynamicImage)> = None;

    for png_blob in find_png_blobs(bytes) {
        let Some(decoded) = decode_icon_blob(png_blob) else {
            continue;
        };
        let width = u64::from(decoded.width());
        let height = u64::from(decoded.height());
        let target = u64::from(size);
        let score = width.abs_diff(target) + height.abs_diff(target);

        if best_match
            .as_ref()
            .is_none_or(|(current_score, _)| score < *current_score)
        {
            best_match = Some((score, decoded));
        }
    }

    best_match.map(|(_, decoded)| decoded)
}

fn find_png_blobs(bytes: &[u8]) -> Vec<&[u8]> {
    if bytes.len() < PNG_SIGNATURE.len() {
        return Vec::new();
    }

    let mut blobs = Vec::new();
    let mut cursor = 0_usize;

    while let Some(start) = find_bytes(bytes, PNG_SIGNATURE, cursor) {
        if let Some(end) = parse_png_end(bytes, start) {
            if let Some(blob) = bytes.get(start..end) {
                blobs.push(blob);
                cursor = end;
                continue;
            }
        }
        cursor = start.saturating_add(1);
    }

    blobs
}

fn parse_png_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut offset = start + PNG_SIGNATURE.len();

    while offset.checked_add(12)? <= bytes.len() {
        let length = u32::from_be_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
            *bytes.get(offset + 2)?,
            *bytes.get(offset + 3)?,
        ]) as usize;
        let chunk_type_offset = offset + 4;
        let chunk_data_offset = chunk_type_offset + 4;
        let chunk_end = chunk_data_offset.checked_add(length)?.checked_add(4)?;

        if chunk_end > bytes.len() {
            return None;
        }

        let chunk_type = bytes.get(chunk_type_offset..chunk_type_offset + 4)?;
        offset = chunk_end;

        if chunk_type == PNG_IEND {
            return Some(chunk_end);
        }
    }

    None
}

fn find_bytes(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    if needle.is_empty() || start >= haystack.len() || needle.len() > haystack.len() {
        return None;
    }

    let end = haystack.len() - needle.len();
    (start..=end).find(|&idx| haystack[idx..idx + needle.len()] == *needle)
}

fn find_ico_blob(bytes: &[u8]) -> Option<&[u8]> {
    if bytes.len() < ICO_HEADER_LEN {
        return None;
    }

    for index in 0..=(bytes.len() - ICO_HEADER_LEN) {
        if bytes[index] != 0
            || bytes[index + 1] != 0
            || bytes[index + 2] != 1
            || bytes[index + 3] != 0
        {
            continue;
        }

        let entry_count = usize::from(u16::from_le_bytes([bytes[index + 4], bytes[index + 5]]));
        if entry_count == 0 || entry_count > MAX_ICON_DIR_ENTRIES {
            continue;
        }

        let table_len = ICO_HEADER_LEN + (entry_count * ICO_DIR_ENTRY_LEN);
        if index
            .checked_add(table_len)
            .is_none_or(|end| end > bytes.len())
        {
            continue;
        }

        let mut end_offset = index + table_len;
        let mut valid_entries = true;

        for entry in 0..entry_count {
            let entry_offset = index + ICO_HEADER_LEN + (entry * ICO_DIR_ENTRY_LEN);
            let image_size = u32::from_le_bytes([
                bytes[entry_offset + 8],
                bytes[entry_offset + 9],
                bytes[entry_offset + 10],
                bytes[entry_offset + 11],
            ]) as usize;
            let image_offset = u32::from_le_bytes([
                bytes[entry_offset + 12],
                bytes[entry_offset + 13],
                bytes[entry_offset + 14],
                bytes[entry_offset + 15],
            ]) as usize;

            let absolute_offset = index + image_offset;
            let image_end = absolute_offset.saturating_add(image_size);
            if image_size == 0
                || image_offset < table_len
                || absolute_offset < index
                || image_end > bytes.len()
            {
                valid_entries = false;
                break;
            }
            end_offset = end_offset.max(image_end);
        }

        if valid_entries {
            return bytes.get(index..end_offset);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{find_ico_blob, find_png_blobs};
    use image::{ImageBuffer, Rgba};

    #[test]
    fn finds_embedded_ico_blob() {
        let bytes = [
            1_u8, 2, 3, 4, 0, 0, 1, 0, 1, 0, 16, 16, 0, 0, 1, 0, 32, 0, 4, 0, 0, 0, 22, 0, 0, 0,
            0xaa, 0xbb, 0xcc, 0xdd,
        ];

        let found = find_ico_blob(&bytes);
        assert!(found.is_some());
    }

    #[test]
    fn rejects_broken_entry_offsets() {
        let bytes = [
            0_u8, 0, 1, 0, 1, 0, 16, 16, 0, 0, 1, 0, 32, 0, 4, 0, 0, 0, 250, 0, 0, 0,
        ];
        assert!(find_ico_blob(&bytes).is_none());
    }

    #[test]
    fn rejects_excessive_ico_entry_counts() {
        let bytes = [0_u8, 0, 1, 0, 0xFF, 0xFF, 16, 16, 0, 0, 1, 0, 32, 0];
        assert!(find_ico_blob(&bytes).is_none());
    }

    #[test]
    fn malformed_blob_fuzz_corpus_does_not_panic() {
        for seed in 0_u8..=255 {
            let mut bytes = vec![0_u8; 64];
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = seed
                    .wrapping_mul(53)
                    .wrapping_add((index as u8).wrapping_mul(19));
            }

            let found = find_ico_blob(&bytes);
            if let Some(blob) = found {
                assert!(!blob.is_empty());
                assert!(blob.len() <= bytes.len());
            }
        }
    }

    #[test]
    fn extracts_embedded_png_blob() {
        let image = ImageBuffer::from_pixel(2, 2, Rgba([255_u8, 0, 0, 255]));
        let mut png_bytes = Vec::new();
        let encoded = image::DynamicImage::ImageRgba8(image).write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        );
        assert!(encoded.is_ok());

        let mut payload = vec![1_u8, 2, 3, 4];
        payload.extend_from_slice(&png_bytes);
        payload.extend_from_slice(&[9_u8, 8, 7, 6]);

        let blobs = find_png_blobs(&payload);
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0], png_bytes.as_slice());
    }
}
