use super::*;

pub(crate) fn encode_constraints(constraints: SurfaceConstraints, out: &mut Vec<u8>) {
    encode_option_size(constraints.min_size, out);
    encode_option_size(constraints.max_size, out);
}

pub(crate) fn decode_constraints(
    cursor: &mut Cursor<'_>,
) -> Result<SurfaceConstraints, IpcCodecError> {
    Ok(SurfaceConstraints {
        min_size: decode_option_size(cursor)?,
        max_size: decode_option_size(cursor)?,
    })
}

pub(crate) fn encode_rect(rect: Rect, out: &mut Vec<u8>) {
    push_i32(out, rect.x);
    push_i32(out, rect.y);
    push_i32(out, rect.width);
    push_i32(out, rect.height);
}

pub(crate) fn decode_rect(cursor: &mut Cursor<'_>) -> Result<Rect, IpcCodecError> {
    Ok(Rect {
        x: cursor.i32()?,
        y: cursor.i32()?,
        width: cursor.i32()?,
        height: cursor.i32()?,
    })
}

pub(crate) fn encode_size(size: Size, out: &mut Vec<u8>) {
    push_i32(out, size.width);
    push_i32(out, size.height);
}

pub(crate) fn decode_size(cursor: &mut Cursor<'_>) -> Result<Size, IpcCodecError> {
    Ok(Size {
        width: cursor.i32()?,
        height: cursor.i32()?,
    })
}

pub(crate) fn encode_option_size(size: Option<Size>, out: &mut Vec<u8>) {
    match size {
        Some(size) => {
            push_u8(out, 1);
            encode_size(size, out);
        }
        None => push_u8(out, 0),
    }
}

pub(crate) fn decode_option_size(cursor: &mut Cursor<'_>) -> Result<Option<Size>, IpcCodecError> {
    match cursor.u8()? {
        0 => Ok(None),
        1 => Ok(Some(decode_size(cursor)?)),
        other => Err(IpcCodecError::InvalidBool {
            field: "option_size",
            value: other,
        }),
    }
}

pub(crate) fn encode_option_rect(rect: Option<Rect>, out: &mut Vec<u8>) {
    match rect {
        Some(rect) => {
            push_u8(out, 1);
            encode_rect(rect, out);
        }
        None => push_u8(out, 0),
    }
}

pub(crate) fn decode_option_rect(cursor: &mut Cursor<'_>) -> Result<Option<Rect>, IpcCodecError> {
    match cursor.u8()? {
        0 => Ok(None),
        1 => Ok(Some(decode_rect(cursor)?)),
        other => Err(IpcCodecError::InvalidBool {
            field: "option_rect",
            value: other,
        }),
    }
}

pub(crate) fn encode_transform(transform: Transform, out: &mut Vec<u8>) {
    for value in transform.matrix {
        push_u32(out, value.to_bits());
    }
}

pub(crate) fn decode_transform(cursor: &mut Cursor<'_>) -> Result<Transform, IpcCodecError> {
    let mut matrix = [0.0; 9];
    for value in &mut matrix {
        *value = f32::from_bits(cursor.u32()?);
    }
    Ok(Transform { matrix })
}
