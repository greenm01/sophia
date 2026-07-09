use super::types::IpcCodecError;

pub(crate) fn push_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

pub(crate) fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn push_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(crate) fn finish(&self) -> Result<(), IpcCodecError> {
        let remaining = self.bytes.len().saturating_sub(self.offset);
        if remaining == 0 {
            Ok(())
        } else {
            Err(IpcCodecError::TrailingBytes(remaining))
        }
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], IpcCodecError> {
        let end = self.offset.checked_add(N).ok_or(IpcCodecError::Truncated)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(IpcCodecError::Truncated)?;
        self.offset = end;
        let mut out = [0; N];
        out.copy_from_slice(slice);
        Ok(out)
    }

    pub(crate) fn slice(&mut self, len: usize) -> Result<&'a [u8], IpcCodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(IpcCodecError::Truncated)?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(IpcCodecError::Truncated)?;
        self.offset = end;
        Ok(slice)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, IpcCodecError> {
        Ok(self.take::<1>()?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, IpcCodecError> {
        Ok(u16::from_le_bytes(self.take()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, IpcCodecError> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, IpcCodecError> {
        Ok(u64::from_le_bytes(self.take()?))
    }

    pub(crate) fn i32(&mut self) -> Result<i32, IpcCodecError> {
        Ok(i32::from_le_bytes(self.take()?))
    }
}
