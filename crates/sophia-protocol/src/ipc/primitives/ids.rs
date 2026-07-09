use super::*;

pub(crate) fn encode_surface_id(id: SurfaceId, out: &mut Vec<u8>) {
    push_u32(out, id.index());
    push_u32(out, id.generation());
}

pub(crate) fn decode_surface_id(cursor: &mut Cursor<'_>) -> Result<SurfaceId, IpcCodecError> {
    Ok(SurfaceId::new(cursor.u32()?, cursor.u32()?))
}

pub(crate) fn encode_workspace_id(id: WorkspaceId, out: &mut Vec<u8>) {
    push_u64(out, id.raw());
}

pub(crate) fn decode_workspace_id(cursor: &mut Cursor<'_>) -> Result<WorkspaceId, IpcCodecError> {
    Ok(WorkspaceId::from_raw(cursor.u64()?))
}

pub(crate) fn encode_output_id(id: OutputId, out: &mut Vec<u8>) {
    push_u64(out, id.raw());
}

pub(crate) fn decode_output_id(cursor: &mut Cursor<'_>) -> Result<OutputId, IpcCodecError> {
    Ok(OutputId::from_raw(cursor.u64()?))
}
