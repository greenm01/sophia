use super::*;

pub(crate) fn check_count(count: usize) -> Result<(), IpcCodecError> {
    if count > SOPHIA_IPC_MAX_ITEMS {
        Err(IpcCodecError::CountTooLarge {
            count,
            max: SOPHIA_IPC_MAX_ITEMS,
        })
    } else {
        Ok(())
    }
}

pub(crate) fn decode_count(cursor: &mut Cursor<'_>) -> Result<usize, IpcCodecError> {
    let count = cursor.u32()? as usize;
    check_count(count)?;
    Ok(count)
}
