use std::path::Path;

use image::imageops::FilterType;

use crate::exe::error::ExeThumbError;
use crate::exe::extractor::ExeIconExtractor;

const ICO_HEADER_LEN: usize = 6;
const ICO_DIR_ENTRY_LEN: usize = 16;
const MAX_ICON_DIR_ENTRIES: usize = 64;

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

    let ico_blob = find_ico_blob(&bytes).ok_or_else(|| ExeThumbError::NoIconResource {
        path: path.to_path_buf(),
    })?;

    let decoded =
        image::load_from_memory(ico_blob).map_err(|source| ExeThumbError::DecodeFailed {
            path: path.to_path_buf(),
            reason: source.to_string(),
        })?;

    let target = decoded.resize(size, size, FilterType::CatmullRom);

    target
        .save(out)
        .map_err(|source| ExeThumbError::DecodeFailed {
            path: out.to_path_buf(),
            reason: source.to_string(),
        })
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
    use super::find_ico_blob;

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
}
