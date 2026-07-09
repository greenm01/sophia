use super::*;

pub(crate) fn encode_optional_text(
    out: &mut Vec<u8>,
    field: &'static str,
    value: Option<&str>,
    max: usize,
) -> Result<(), IpcCodecError> {
    match value {
        Some(value) => {
            if value.len() > max {
                return Err(IpcCodecError::TextTooLarge {
                    field,
                    len: value.len(),
                    max,
                });
            }
            push_u8(out, 1);
            push_u16(out, value.len() as u16);
            out.extend_from_slice(value.as_bytes());
        }
        None => push_u8(out, 0),
    }

    Ok(())
}

pub(crate) fn decode_optional_text(
    cursor: &mut Cursor<'_>,
    field: &'static str,
    max: usize,
) -> Result<Option<String>, IpcCodecError> {
    match cursor.u8()? {
        0 => Ok(None),
        1 => {
            let len = cursor.u16()? as usize;
            if len > max {
                return Err(IpcCodecError::TextTooLarge { field, len, max });
            }
            let bytes = cursor.slice(len)?;
            let text = core::str::from_utf8(bytes)
                .map_err(|_| IpcCodecError::InvalidUtf8 { field })?
                .to_owned();
            Ok(Some(text))
        }
        other => Err(IpcCodecError::InvalidBool {
            field,
            value: other,
        }),
    }
}
