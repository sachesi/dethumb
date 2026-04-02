use std::io::{Read, Seek, SeekFrom};

pub const MAX_SECTION_COUNT: u16 = 96;
pub const MIN_PE_HEADER_SIZE: u64 = 24;

const DOS_HEADER_LEN: usize = 64;
const PE_SIGNATURE_LEN: usize = 4;
const COFF_HEADER_LEN: usize = 20;
const KNOWN_MACHINE_TYPES: [u16; 4] = [0x014c, 0x01c0, 0x8664, 0xaa64];

pub fn validate_executable_header(
    file: &mut std::fs::File,
    file_len: u64,
) -> std::io::Result<bool> {
    let mut dos_header = [0_u8; DOS_HEADER_LEN];
    let bytes_read = file.read(&mut dos_header)?;
    if bytes_read < dos_header.len() || dos_header[0..2] != [b'M', b'Z'] {
        return Ok(false);
    }

    let pe_header_offset = u32::from_le_bytes([
        dos_header[0x3c],
        dos_header[0x3d],
        dos_header[0x3e],
        dos_header[0x3f],
    ]) as u64;

    if pe_header_offset < DOS_HEADER_LEN as u64
        || pe_header_offset
            .checked_add(MIN_PE_HEADER_SIZE)
            .is_none_or(|end| end > file_len)
    {
        return Ok(false);
    }

    file.seek(SeekFrom::Start(pe_header_offset))?;
    let mut signature = [0_u8; PE_SIGNATURE_LEN];
    file.read_exact(&mut signature)?;
    if signature != [b'P', b'E', 0, 0] {
        return Ok(false);
    }

    let mut coff_header = [0_u8; COFF_HEADER_LEN];
    file.read_exact(&mut coff_header)?;

    let machine = u16::from_le_bytes([coff_header[0], coff_header[1]]);
    if !KNOWN_MACHINE_TYPES.contains(&machine) {
        return Ok(false);
    }

    let section_count = u16::from_le_bytes([coff_header[2], coff_header[3]]);
    if section_count == 0 || section_count > MAX_SECTION_COUNT {
        return Ok(false);
    }

    let optional_header_size = u16::from_le_bytes([coff_header[16], coff_header[17]]) as u64;
    if optional_header_size < 2 {
        return Ok(false);
    }

    let optional_header_offset = pe_header_offset + MIN_PE_HEADER_SIZE;
    if optional_header_offset
        .checked_add(optional_header_size)
        .is_none_or(|end| end > file_len)
    {
        return Ok(false);
    }

    file.seek(SeekFrom::Start(optional_header_offset))?;
    let mut optional_magic = [0_u8; 2];
    file.read_exact(&mut optional_magic)?;
    let optional_magic = u16::from_le_bytes(optional_magic);
    if optional_magic != 0x010b && optional_magic != 0x020b {
        return Ok(false);
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::validate_executable_header;
    use std::io::{Seek, SeekFrom, Write};
    use tempfile::NamedTempFile;

    fn write_minimal_valid_pe() -> NamedTempFile {
        let temp = NamedTempFile::new();
        assert!(temp.is_ok());
        let Ok(mut temp) = temp else {
            panic!("temporary file should be created");
        };

        let mut bytes = vec![0_u8; 512];
        bytes[0] = b'M';
        bytes[1] = b'Z';

        let pe_offset: u32 = 0x80;
        bytes[0x3c..0x40].copy_from_slice(&pe_offset.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        bytes[0x84..0x86].copy_from_slice(&0x8664_u16.to_le_bytes());
        bytes[0x86..0x88].copy_from_slice(&4_u16.to_le_bytes());
        bytes[0x94..0x96].copy_from_slice(&0x00F0_u16.to_le_bytes());
        bytes[0x98..0x9a].copy_from_slice(&0x020b_u16.to_le_bytes());

        let write_result = temp.write_all(&bytes);
        assert!(write_result.is_ok());
        temp
    }

    #[test]
    fn validates_minimal_pe64_headers() {
        let mut temp = write_minimal_valid_pe();
        let metadata = temp.path().metadata();
        assert!(metadata.is_ok());
        let Ok(metadata) = metadata else {
            panic!("metadata should be available");
        };

        let seek_result = temp.seek(SeekFrom::Start(0));
        assert!(seek_result.is_ok());
        let valid = validate_executable_header(temp.as_file_mut(), metadata.len());
        assert!(valid.is_ok());
        assert!(matches!(valid, Ok(true)));
    }

    #[test]
    fn rejects_unknown_machine_type() {
        let mut temp = write_minimal_valid_pe();
        let patch_result = temp.as_file_mut().seek(SeekFrom::Start(0x84));
        assert!(patch_result.is_ok());
        assert!(
            temp.as_file_mut()
                .write_all(&0x9999_u16.to_le_bytes())
                .is_ok()
        );

        let metadata = temp.path().metadata();
        assert!(metadata.is_ok());
        let Ok(metadata) = metadata else {
            panic!("metadata should be available");
        };
        assert!(temp.as_file_mut().seek(SeekFrom::Start(0)).is_ok());
        let valid = validate_executable_header(temp.as_file_mut(), metadata.len());
        assert!(matches!(valid, Ok(false)));
    }
}
