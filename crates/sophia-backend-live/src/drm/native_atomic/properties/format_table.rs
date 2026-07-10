#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativePlaneFormatModifierTable {
    modifiers: Vec<drm::buffer::DrmModifier>,
}

impl LibdrmNativePlaneFormatModifierTable {
    pub fn parse_for_format(
        blob: &[u8],
        format: drm::buffer::DrmFourcc,
    ) -> LibdrmNativePlaneFormatModifierTableParseResult {
        let Some(header) = FormatModifierBlobHeader::parse(blob) else {
            return LibdrmNativePlaneFormatModifierTableParseResult {
                status: LibdrmNativePlaneFormatModifierTableParseStatus::Malformed,
                table: None,
            };
        };
        if header.version != FORMAT_BLOB_CURRENT {
            return LibdrmNativePlaneFormatModifierTableParseResult {
                status: LibdrmNativePlaneFormatModifierTableParseStatus::UnsupportedVersion,
                table: None,
            };
        }

        let Some(formats) = read_u32_table(blob, header.formats_offset, header.count_formats)
        else {
            return LibdrmNativePlaneFormatModifierTableParseResult {
                status: LibdrmNativePlaneFormatModifierTableParseStatus::Malformed,
                table: None,
            };
        };
        let Some(modifiers) =
            read_modifier_table(blob, header.modifiers_offset, header.count_modifiers)
        else {
            return LibdrmNativePlaneFormatModifierTableParseResult {
                status: LibdrmNativePlaneFormatModifierTableParseStatus::Malformed,
                table: None,
            };
        };

        let Some(format_index) = formats
            .iter()
            .position(|candidate| *candidate == format as u32)
        else {
            return LibdrmNativePlaneFormatModifierTableParseResult {
                status: LibdrmNativePlaneFormatModifierTableParseStatus::FormatUnsupported,
                table: Some(Self {
                    modifiers: Vec::new(),
                }),
            };
        };

        let mut reduced = Vec::new();
        for modifier in modifiers {
            if modifier.applies_to_format_index(format_index) {
                let drm_modifier = drm::buffer::DrmModifier::from(modifier.modifier);
                if !matches!(drm_modifier, drm::buffer::DrmModifier::Invalid) {
                    reduced.push(drm_modifier);
                }
            }
        }
        dedup_modifiers(&mut reduced);

        LibdrmNativePlaneFormatModifierTableParseResult {
            status: if reduced.is_empty() {
                LibdrmNativePlaneFormatModifierTableParseStatus::ModifierUnsupported
            } else {
                LibdrmNativePlaneFormatModifierTableParseStatus::Parsed
            },
            table: Some(Self { modifiers: reduced }),
        }
    }

    pub fn modifiers(&self) -> &[drm::buffer::DrmModifier] {
        &self.modifiers
    }

    pub fn reduced_status(&self) -> LibdrmNativePlaneFormatModifierSupportStatus {
        if self
            .modifiers
            .iter()
            .any(|modifier| matches!(modifier, drm::buffer::DrmModifier::Linear))
        {
            LibdrmNativePlaneFormatModifierSupportStatus::Linear
        } else if self.modifiers.is_empty() {
            LibdrmNativePlaneFormatModifierSupportStatus::Unsupported
        } else {
            LibdrmNativePlaneFormatModifierSupportStatus::NonLinear
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibdrmNativePlaneFormatModifierTableParseResult {
    pub status: LibdrmNativePlaneFormatModifierTableParseStatus,
    pub table: Option<LibdrmNativePlaneFormatModifierTable>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePlaneFormatModifierTableParseStatus {
    Parsed,
    UnsupportedVersion,
    FormatUnsupported,
    ModifierUnsupported,
    Malformed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibdrmNativePlaneFormatModifierSupportStatus {
    Linear,
    NonLinear,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FormatModifierBlobHeader {
    version: u32,
    count_formats: u32,
    formats_offset: u32,
    count_modifiers: u32,
    modifiers_offset: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FormatModifierRecord {
    formats: u64,
    offset: u32,
    modifier: u64,
}

impl FormatModifierRecord {
    fn applies_to_format_index(self, format_index: usize) -> bool {
        let Ok(format_index) = u32::try_from(format_index) else {
            return false;
        };
        if format_index < self.offset {
            return false;
        }
        let bit = format_index - self.offset;
        bit < 64 && (self.formats & (1u64 << bit)) != 0
    }
}

impl FormatModifierBlobHeader {
    fn parse(blob: &[u8]) -> Option<Self> {
        Some(Self {
            version: read_u32(blob, 0)?,
            count_formats: read_u32(blob, 8)?,
            formats_offset: read_u32(blob, 12)?,
            count_modifiers: read_u32(blob, 16)?,
            modifiers_offset: read_u32(blob, 20)?,
        })
    }
}

fn read_u32_table(blob: &[u8], offset: u32, count: u32) -> Option<Vec<u32>> {
    let offset = usize::try_from(offset).ok()?;
    let count = usize::try_from(count).ok()?;
    checked_table_end(offset, count, 4, blob.len())?;

    let mut values = Vec::with_capacity(count);
    let mut index = 0;
    while index < count {
        values.push(read_u32(blob, offset + index * 4)?);
        index += 1;
    }
    Some(values)
}

fn read_modifier_table(blob: &[u8], offset: u32, count: u32) -> Option<Vec<FormatModifierRecord>> {
    let offset = usize::try_from(offset).ok()?;
    let count = usize::try_from(count).ok()?;
    checked_table_end(offset, count, FORMAT_MODIFIER_RECORD_SIZE, blob.len())?;

    let mut values = Vec::with_capacity(count);
    let mut index = 0;
    while index < count {
        let base = offset + index * FORMAT_MODIFIER_RECORD_SIZE;
        values.push(FormatModifierRecord {
            formats: read_u64(blob, base)?,
            offset: read_u32(blob, base + 8)?,
            modifier: read_u64(blob, base + 16)?,
        });
        index += 1;
    }
    Some(values)
}

fn checked_table_end(offset: usize, count: usize, stride: usize, len: usize) -> Option<usize> {
    let bytes = count.checked_mul(stride)?;
    let end = offset.checked_add(bytes)?;
    (end <= len).then_some(end)
}

fn read_u32(blob: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = blob.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_ne_bytes(bytes))
}

fn read_u64(blob: &[u8], offset: usize) -> Option<u64> {
    let bytes: [u8; 8] = blob.get(offset..offset + 8)?.try_into().ok()?;
    Some(u64::from_ne_bytes(bytes))
}

fn dedup_modifiers(modifiers: &mut Vec<drm::buffer::DrmModifier>) {
    let mut deduped = Vec::with_capacity(modifiers.len());
    for modifier in modifiers.drain(..) {
        if !deduped.contains(&modifier) {
            deduped.push(modifier);
        }
    }
    *modifiers = deduped;
}

const FORMAT_BLOB_CURRENT: u32 = 1;
const FORMAT_MODIFIER_RECORD_SIZE: usize = 24;
