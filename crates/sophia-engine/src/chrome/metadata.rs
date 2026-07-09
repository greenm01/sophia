use crate::prelude::*;

#[derive(Clone, Debug, Default)]
pub struct ChromeBroker {
    descriptors: BTreeMap<SurfaceId, ChromeDescriptor>,
}

pub const MAX_CHROME_LABEL_LEN: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SanitizedChromeMetadata {
    pub surface: SurfaceId,
    pub label: Option<String>,
    pub label_redacted: bool,
    pub icon: Option<IconTokenId>,
    pub trust_level: TrustLevel,
    pub attention: AttentionState,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataChromeUpdate {
    Upserted { surface: SurfaceId },
    Removed { surface: SurfaceId },
    Rejected(MetadataChromeRejectReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataChromeRejectReason {
    InvalidSurface,
    InvalidLabel,
    StaleGeneration,
}

impl ChromeBroker {
    pub fn upsert(&mut self, descriptor: ChromeDescriptor) {
        debug!(
            surface_index = descriptor.surface.index(),
            surface_generation = descriptor.surface.generation(),
            descriptor_generation = descriptor.generation,
            has_label = descriptor.label.is_some(),
            has_icon = descriptor.icon.is_some(),
            trust_level = ?descriptor.trust_level,
            attention = ?descriptor.attention,
            "upserting chrome descriptor"
        );
        self.descriptors.insert(descriptor.surface, descriptor);
    }

    pub fn apply_metadata(&mut self, metadata: SanitizedChromeMetadata) -> MetadataChromeUpdate {
        let surface = metadata.surface;
        let generation = metadata.generation;
        let Ok(descriptor) = chrome_descriptor_from_metadata(metadata) else {
            warn!(
                surface_index = surface.index(),
                surface_generation = surface.generation(),
                metadata_generation = generation,
                "rejected sanitized chrome metadata with invalid label"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidLabel);
        };

        if !descriptor.surface.is_valid() {
            warn!(
                surface_index = descriptor.surface.index(),
                surface_generation = descriptor.surface.generation(),
                metadata_generation = descriptor.generation,
                "rejected sanitized chrome metadata with invalid surface"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidSurface);
        }

        if self
            .get(descriptor.surface)
            .is_some_and(|existing| existing.generation > descriptor.generation)
        {
            warn!(
                surface_index = descriptor.surface.index(),
                surface_generation = descriptor.surface.generation(),
                metadata_generation = descriptor.generation,
                "rejected stale sanitized chrome metadata"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::StaleGeneration);
        }

        let surface = descriptor.surface;
        self.upsert(descriptor);
        MetadataChromeUpdate::Upserted { surface }
    }

    pub fn remove_metadata(&mut self, surface: SurfaceId, generation: u64) -> MetadataChromeUpdate {
        if !surface.is_valid() {
            warn!(
                surface_index = surface.index(),
                surface_generation = surface.generation(),
                metadata_generation = generation,
                "rejected chrome descriptor removal with invalid surface"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::InvalidSurface);
        }

        if self
            .get(surface)
            .is_some_and(|existing| existing.generation > generation)
        {
            warn!(
                surface_index = surface.index(),
                surface_generation = surface.generation(),
                metadata_generation = generation,
                "rejected stale chrome descriptor removal"
            );
            return MetadataChromeUpdate::Rejected(MetadataChromeRejectReason::StaleGeneration);
        }

        self.remove_surface(surface);
        debug!(
            surface_index = surface.index(),
            surface_generation = surface.generation(),
            metadata_generation = generation,
            "removed chrome descriptor metadata"
        );
        MetadataChromeUpdate::Removed { surface }
    }

    pub fn get(&self, surface: SurfaceId) -> Option<&ChromeDescriptor> {
        self.descriptors.get(&surface)
    }

    pub fn remove_surface(&mut self, surface: SurfaceId) -> Option<ChromeDescriptor> {
        self.descriptors.remove(&surface)
    }

    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}

fn chrome_descriptor_from_metadata(
    metadata: SanitizedChromeMetadata,
) -> Result<ChromeDescriptor, MetadataChromeRejectReason> {
    let label = metadata
        .label
        .map(|text| {
            if valid_chrome_label(&text) {
                Ok(DisplayLabel {
                    text,
                    redacted: metadata.label_redacted,
                })
            } else {
                Err(MetadataChromeRejectReason::InvalidLabel)
            }
        })
        .transpose()?;

    Ok(ChromeDescriptor {
        surface: metadata.surface,
        label,
        icon: metadata.icon,
        trust_level: metadata.trust_level,
        attention: metadata.attention,
        generation: metadata.generation,
    })
}

fn valid_chrome_label(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_CHROME_LABEL_LEN && !text.chars().any(char::is_control)
}
