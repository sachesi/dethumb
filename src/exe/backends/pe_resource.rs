use std::path::Path;

use image::imageops::FilterType;
use pelite::PeFile;
use pelite::resources::FindError;

use crate::exe::error::ExeThumbError;
use crate::exe::extractor::ExeIconExtractor;

const ICO_HEADER_LEN: usize = 6;
const ICO_DIR_ENTRY_LEN: usize = 16;
const MAX_ICON_DIR_ENTRIES: usize = 64;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PNG_IEND: &[u8; 4] = b"IEND";
const PE_SIGNATURE_OFFSET: usize = 0x3c;
const PE_FILE_HEADER_LEN: usize = 20;
const PE_SECTION_HEADER_LEN: usize = 40;
const PE_DATA_DIRECTORY_LEN: usize = 8;
const PE_RESOURCE_DIRECTORY_INDEX: usize = 2;
const RESOURCE_DIRECTORY_TABLE_LEN: usize = 16;
const RESOURCE_DIRECTORY_ENTRY_LEN: usize = 8;
const RESOURCE_DATA_ENTRY_LEN: usize = 16;
const RT_ICON: u32 = 3;
const RT_GROUP_ICON: u32 = 14;

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

    let decoded = find_best_group_icon(&bytes, size)
        .or_else(|| find_best_png_icon(&bytes, size))
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

#[derive(Clone, Copy)]
struct SectionHeader {
    virtual_address: u32,
    virtual_size: u32,
    raw_ptr: u32,
    raw_size: u32,
}

#[derive(Clone)]
struct GroupIconEntry {
    width: u8,
    height: u8,
    color_count: u8,
    reserved: u8,
    planes: u16,
    bit_count: u16,
    bytes_in_res: u32,
    icon_id: u16,
}

fn find_best_group_icon(bytes: &[u8], size: u32) -> Option<image::DynamicImage> {
    find_best_group_icon_pelite(bytes, size).or_else(|| find_best_group_icon_manual(bytes, size))
}

fn find_best_group_icon_pelite(bytes: &[u8], size: u32) -> Option<image::DynamicImage> {
    let pe = PeFile::from_bytes(bytes).ok()?;
    let resources = pe.resources().ok()?;
    let mut best: Option<(u64, image::DynamicImage)> = None;

    for icon_result in resources.icons() {
        let (_name, group_icon) = match icon_result {
            Ok(icon) => icon,
            Err(FindError::NotFound) => continue,
            Err(_) => continue,
        };

        let mut ico = Vec::new();
        if group_icon.write(&mut ico).is_err() {
            continue;
        }
        let Some(decoded) = decode_icon_blob(&ico) else {
            continue;
        };

        let score = u64::from(decoded.width()).abs_diff(u64::from(size))
            + u64::from(decoded.height()).abs_diff(u64::from(size));
        if best.as_ref().is_none_or(|(current, _)| score < *current) {
            best = Some((score, decoded));
        }
    }

    best.map(|(_, decoded)| decoded)
}

fn find_best_group_icon_manual(bytes: &[u8], size: u32) -> Option<image::DynamicImage> {
    let pe_offset = usize::try_from(read_u32_le(bytes, PE_SIGNATURE_OFFSET)?).ok()?;
    if bytes.get(pe_offset..pe_offset + 4)? != b"PE\0\0" {
        return None;
    }

    let coff_offset = pe_offset + 4;
    let number_of_sections = usize::from(read_u16_le(bytes, coff_offset + 2)?);
    let optional_header_size = usize::from(read_u16_le(bytes, coff_offset + 16)?);
    let optional_header_offset = coff_offset + PE_FILE_HEADER_LEN;
    let sections_offset = optional_header_offset + optional_header_size;

    let section_headers = read_section_headers(bytes, sections_offset, number_of_sections)?;
    let resource_directory = read_resource_directory(bytes, optional_header_offset)?;
    let resource_root = rva_to_file_offset(resource_directory.0, &section_headers)?;

    let icon_blobs =
        collect_icon_resource_blobs(bytes, resource_root, resource_directory.1, &section_headers)?;
    let groups =
        collect_group_icon_blobs(bytes, resource_root, resource_directory.1, &section_headers)?;

    let mut best: Option<(u64, image::DynamicImage)> = None;
    for group_blob in groups {
        let Some(ico_blob) = build_ico_from_group(&group_blob, &icon_blobs) else {
            continue;
        };
        let Some(decoded) = decode_icon_blob(&ico_blob) else {
            continue;
        };

        let score = u64::from(decoded.width()).abs_diff(u64::from(size))
            + u64::from(decoded.height()).abs_diff(u64::from(size));

        if best.as_ref().is_none_or(|(current, _)| score < *current) {
            best = Some((score, decoded));
        }
    }

    best.map(|(_, image)| image)
}

fn read_resource_directory(bytes: &[u8], optional_header_offset: usize) -> Option<(u32, u32)> {
    let magic = read_u16_le(bytes, optional_header_offset)?;
    let data_directory_offset = if magic == 0x010b {
        optional_header_offset + 96
    } else if magic == 0x020b {
        optional_header_offset + 112
    } else {
        return None;
    };

    let entry_offset =
        data_directory_offset + (PE_RESOURCE_DIRECTORY_INDEX * PE_DATA_DIRECTORY_LEN);
    let rva = read_u32_le(bytes, entry_offset)?;
    let size = read_u32_le(bytes, entry_offset + 4)?;
    if rva == 0 || size == 0 {
        return None;
    }

    Some((rva, size))
}

fn read_section_headers(
    bytes: &[u8],
    mut offset: usize,
    number_of_sections: usize,
) -> Option<Vec<SectionHeader>> {
    let mut sections = Vec::with_capacity(number_of_sections);
    for _ in 0..number_of_sections {
        if offset.checked_add(PE_SECTION_HEADER_LEN)? > bytes.len() {
            return None;
        }
        let virtual_size = read_u32_le(bytes, offset + 8)?;
        let virtual_address = read_u32_le(bytes, offset + 12)?;
        let raw_size = read_u32_le(bytes, offset + 16)?;
        let raw_ptr = read_u32_le(bytes, offset + 20)?;
        sections.push(SectionHeader {
            virtual_address,
            virtual_size,
            raw_ptr,
            raw_size,
        });
        offset += PE_SECTION_HEADER_LEN;
    }
    Some(sections)
}

fn collect_icon_resource_blobs(
    bytes: &[u8],
    resource_root: usize,
    resource_size: u32,
    sections: &[SectionHeader],
) -> Option<std::collections::BTreeMap<u16, Vec<u8>>> {
    let mut icons = std::collections::BTreeMap::new();
    for (_lang, data) in
        collect_resource_data_for_type(bytes, resource_root, resource_size, RT_ICON)?
    {
        let icon_id = u16::try_from(data.id).ok()?;
        let blob = read_resource_data_entry(bytes, data.data_offset, sections)?;
        icons.insert(icon_id, blob.to_vec());
    }
    Some(icons)
}

fn collect_group_icon_blobs(
    bytes: &[u8],
    resource_root: usize,
    resource_size: u32,
    sections: &[SectionHeader],
) -> Option<Vec<Vec<u8>>> {
    let mut groups = Vec::new();
    for (_lang, data) in
        collect_resource_data_for_type(bytes, resource_root, resource_size, RT_GROUP_ICON)?
    {
        let blob = read_resource_data_entry(bytes, data.data_offset, sections)?;
        groups.push(blob.to_vec());
    }
    Some(groups)
}

#[derive(Clone, Copy)]
struct ResourceLeaf {
    id: u32,
    data_offset: usize,
}

fn collect_resource_data_for_type(
    bytes: &[u8],
    root_offset: usize,
    resource_size: u32,
    target_type: u32,
) -> Option<Vec<(u32, ResourceLeaf)>> {
    let type_entries = read_resource_entries(bytes, root_offset)?;
    let type_entry = type_entries
        .into_iter()
        .find(|entry| !entry.name_is_string && entry.id == target_type)?;
    let type_dir = resolve_resource_subdir(root_offset, type_entry.offset_to_data, resource_size)?;
    let name_entries = read_resource_entries(bytes, type_dir)?;
    let mut leaves = Vec::new();

    for name_entry in name_entries {
        collect_resource_leaves_from_entry(
            bytes,
            root_offset,
            resource_size,
            name_entry.id,
            name_entry.offset_to_data,
            0,
            &mut leaves,
        )?;
    }

    Some(leaves)
}

fn collect_resource_leaves_from_entry(
    bytes: &[u8],
    root_offset: usize,
    resource_size: u32,
    name_id: u32,
    offset_to_data: u32,
    depth: usize,
    out: &mut Vec<(u32, ResourceLeaf)>,
) -> Option<()> {
    if depth > 2 {
        return Some(());
    }

    if (offset_to_data & 0x8000_0000) == 0 {
        let data_offset = resolve_resource_data_entry(root_offset, offset_to_data, resource_size)?;
        out.push((
            0,
            ResourceLeaf {
                id: name_id,
                data_offset,
            },
        ));
        return Some(());
    }

    let dir_offset = resolve_resource_subdir(root_offset, offset_to_data, resource_size)?;
    let entries = read_resource_entries(bytes, dir_offset)?;
    for entry in entries {
        if (entry.offset_to_data & 0x8000_0000) == 0 {
            let data_offset =
                resolve_resource_data_entry(root_offset, entry.offset_to_data, resource_size)?;
            out.push((
                entry.id,
                ResourceLeaf {
                    id: name_id,
                    data_offset,
                },
            ));
        } else {
            collect_resource_leaves_from_entry(
                bytes,
                root_offset,
                resource_size,
                name_id,
                entry.offset_to_data,
                depth + 1,
                out,
            )?;
        }
    }

    Some(())
}

#[derive(Clone, Copy)]
struct ResourceDirectoryEntry {
    id: u32,
    name_is_string: bool,
    offset_to_data: u32,
}

fn read_resource_entries(bytes: &[u8], dir_offset: usize) -> Option<Vec<ResourceDirectoryEntry>> {
    if dir_offset.checked_add(RESOURCE_DIRECTORY_TABLE_LEN)? > bytes.len() {
        return None;
    }
    let named_count = usize::from(read_u16_le(bytes, dir_offset + 12)?);
    let id_count = usize::from(read_u16_le(bytes, dir_offset + 14)?);
    let count = named_count.checked_add(id_count)?;
    let mut entries = Vec::with_capacity(count);
    let mut offset = dir_offset + RESOURCE_DIRECTORY_TABLE_LEN;
    for _ in 0..count {
        if offset.checked_add(RESOURCE_DIRECTORY_ENTRY_LEN)? > bytes.len() {
            return None;
        }
        let name_raw = read_u32_le(bytes, offset)?;
        let data_raw = read_u32_le(bytes, offset + 4)?;
        entries.push(ResourceDirectoryEntry {
            id: name_raw & 0x7fff_ffff,
            name_is_string: (name_raw & 0x8000_0000) != 0,
            offset_to_data: data_raw,
        });
        offset += RESOURCE_DIRECTORY_ENTRY_LEN;
    }
    Some(entries)
}

fn resolve_resource_subdir(
    root_offset: usize,
    raw_offset: u32,
    resource_size: u32,
) -> Option<usize> {
    if (raw_offset & 0x8000_0000) == 0 {
        return None;
    }
    let relative = usize::try_from(raw_offset & 0x7fff_ffff).ok()?;
    let absolute = root_offset.checked_add(relative)?;
    let max = root_offset.checked_add(usize::try_from(resource_size).ok()?)?;
    if absolute >= max {
        return None;
    }
    Some(absolute)
}

fn resolve_resource_data_entry(
    root_offset: usize,
    raw_offset: u32,
    resource_size: u32,
) -> Option<usize> {
    if (raw_offset & 0x8000_0000) != 0 {
        return None;
    }
    let relative = usize::try_from(raw_offset).ok()?;
    let absolute = root_offset.checked_add(relative)?;
    let max = root_offset.checked_add(usize::try_from(resource_size).ok()?)?;
    let end = absolute.checked_add(RESOURCE_DATA_ENTRY_LEN)?;
    if end > max {
        return None;
    }
    Some(absolute)
}

fn read_resource_data_entry<'a>(
    bytes: &'a [u8],
    data_entry_offset: usize,
    sections: &[SectionHeader],
) -> Option<&'a [u8]> {
    let data_rva = read_u32_le(bytes, data_entry_offset)?;
    let data_size = usize::try_from(read_u32_le(bytes, data_entry_offset + 4)?).ok()?;
    let data_offset = rva_to_file_offset(data_rva, sections)?;
    bytes.get(data_offset..data_offset.checked_add(data_size)?)
}

fn rva_to_file_offset(rva: u32, sections: &[SectionHeader]) -> Option<usize> {
    for section in sections {
        let size = section.virtual_size.max(section.raw_size);
        let start = section.virtual_address;
        let end = start.checked_add(size)?;
        if (start..end).contains(&rva) {
            let within = rva.checked_sub(start)?;
            let file_offset = section.raw_ptr.checked_add(within)?;
            return usize::try_from(file_offset).ok();
        }
    }
    None
}

fn build_ico_from_group(
    group_blob: &[u8],
    icons_by_id: &std::collections::BTreeMap<u16, Vec<u8>>,
) -> Option<Vec<u8>> {
    let count = usize::from(read_u16_le(group_blob, 4)?);
    if count == 0 {
        return None;
    }
    let entries_size = count.checked_mul(14)?;
    if 6_usize.checked_add(entries_size)? > group_blob.len() {
        return None;
    }

    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let offset = 6 + (i * 14);
        entries.push(GroupIconEntry {
            width: *group_blob.get(offset)?,
            height: *group_blob.get(offset + 1)?,
            color_count: *group_blob.get(offset + 2)?,
            reserved: *group_blob.get(offset + 3)?,
            planes: read_u16_le(group_blob, offset + 4)?,
            bit_count: read_u16_le(group_blob, offset + 6)?,
            bytes_in_res: read_u32_le(group_blob, offset + 8)?,
            icon_id: read_u16_le(group_blob, offset + 12)?,
        });
    }

    let mut images = Vec::new();
    for entry in &entries {
        let icon = icons_by_id.get(&entry.icon_id)?;
        images.push(icon.as_slice());
    }

    let mut output = Vec::new();
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&(u16::try_from(count).ok()?).to_le_bytes());

    let mut image_offset = 6 + (count * 16);
    for (entry, image) in entries.iter().zip(&images) {
        output.push(entry.width);
        output.push(entry.height);
        output.push(entry.color_count);
        output.push(entry.reserved);
        output.extend_from_slice(&entry.planes.to_le_bytes());
        output.extend_from_slice(&entry.bit_count.to_le_bytes());
        output.extend_from_slice(&(u32::try_from(image.len()).ok()?).to_le_bytes());
        output.extend_from_slice(&(u32::try_from(image_offset).ok()?).to_le_bytes());
        let _ = entry.bytes_in_res;
        image_offset = image_offset.checked_add(image.len())?;
    }

    for image in images {
        output.extend_from_slice(image);
    }
    Some(output)
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
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
    use super::{build_ico_from_group, decode_icon_blob, find_ico_blob, find_png_blobs};
    use image::{ImageBuffer, Rgba};
    use std::collections::BTreeMap;

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

    #[test]
    fn rebuilds_single_entry_group_icon_from_twenty_byte_header() {
        let icon_payload = vec![1_u8, 2, 3, 4];
        let mut icons = BTreeMap::new();
        icons.insert(1_u16, icon_payload.clone());

        // GRPICONDIR (6) + 1x GRPICONDIRENTRY (14) = 20 bytes
        let mut group = Vec::new();
        group.extend_from_slice(&0_u16.to_le_bytes()); // reserved
        group.extend_from_slice(&1_u16.to_le_bytes()); // type icon
        group.extend_from_slice(&1_u16.to_le_bytes()); // count
        group.push(16); // width
        group.push(16); // height
        group.push(0); // color count
        group.push(0); // reserved
        group.extend_from_slice(&1_u16.to_le_bytes()); // planes
        group.extend_from_slice(&32_u16.to_le_bytes()); // bit count
        group.extend_from_slice(&(u32::try_from(icon_payload.len()).unwrap_or(0)).to_le_bytes());
        group.extend_from_slice(&1_u16.to_le_bytes()); // nID
        assert_eq!(group.len(), 20);

        let rebuilt = build_ico_from_group(&group, &icons);
        assert!(rebuilt.is_some());
        let rebuilt = rebuilt.unwrap_or_default();

        assert_eq!(&rebuilt[0..2], &0_u16.to_le_bytes());
        assert_eq!(&rebuilt[2..4], &1_u16.to_le_bytes());
        assert_eq!(&rebuilt[4..6], &1_u16.to_le_bytes());
        assert_eq!(&rebuilt[22..26], &icon_payload);
    }

    #[test]
    fn decodes_png_backed_single_entry_group_icon() {
        let image = ImageBuffer::from_pixel(2, 2, Rgba([255_u8, 0, 0, 255]));
        let mut png_bytes = Vec::new();
        let encoded = image::DynamicImage::ImageRgba8(image).write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        );
        assert!(encoded.is_ok());

        let mut icons = BTreeMap::new();
        icons.insert(1_u16, png_bytes.clone());

        let mut group = Vec::new();
        group.extend_from_slice(&0_u16.to_le_bytes());
        group.extend_from_slice(&1_u16.to_le_bytes());
        group.extend_from_slice(&1_u16.to_le_bytes());
        group.push(2);
        group.push(2);
        group.push(0);
        group.push(0);
        group.extend_from_slice(&1_u16.to_le_bytes());
        group.extend_from_slice(&32_u16.to_le_bytes());
        group.extend_from_slice(&(u32::try_from(png_bytes.len()).unwrap_or(0)).to_le_bytes());
        group.extend_from_slice(&1_u16.to_le_bytes());

        let rebuilt = build_ico_from_group(&group, &icons);
        assert!(rebuilt.is_some());
        let decoded = decode_icon_blob(&rebuilt.unwrap_or_default());
        assert!(decoded.is_some());
    }
}
