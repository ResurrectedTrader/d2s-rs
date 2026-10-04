//! Save file checksum.

/// Offset of the checksum field in the save file header.
const CHECKSUM_OFFSET: usize = 0x0C;

/// Calculates the checksum of a save file, treating the checksum field itself as zero.
pub fn calculate(data: &[u8]) -> u32 {
    let mut checksum = 0u32;
    for (i, &b) in data.iter().enumerate() {
        let b = if (CHECKSUM_OFFSET..CHECKSUM_OFFSET + 4).contains(&i) { 0 } else { b };
        checksum = checksum.rotate_left(1).wrapping_add(b as u32);
    }
    checksum
}

/// Verifies the checksum stored in a save file.
pub fn verify(data: &[u8]) -> bool {
    match data.get(CHECKSUM_OFFSET..CHECKSUM_OFFSET + 4) {
        Some(stored) => u32::from_le_bytes(stored.try_into().unwrap()) == calculate(data),
        None => false,
    }
}

/// Recalculates and stores the checksum of a save file.
///
/// # Errors
/// Fails if `data` is too short to contain a checksum field.
pub fn update(data: &mut [u8]) -> crate::Result<()> {
    if data.len() < CHECKSUM_OFFSET + 4 {
        return Err(crate::Error::InvalidArgument("data too short for checksum".into()));
    }
    let checksum = calculate(data);
    data[CHECKSUM_OFFSET..CHECKSUM_OFFSET + 4].copy_from_slice(&checksum.to_le_bytes());
    Ok(())
}
