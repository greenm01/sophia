use std::collections::BTreeMap;

use sophia_protocol::{
    AuthorityKind, AuthoritySurface, NamespaceId, Rect, SurfaceConstraints, SurfaceId,
};

use crate::{XAuthorityAccessError, XMapState, XResourceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XWindowRecord {
    pub id: XResourceId,
    pub surface: SurfaceId,
    pub namespace: NamespaceId,
    pub map_state: XMapState,
    pub geometry: Rect,
    pub constraints: SurfaceConstraints,
    pub generation: u64,
}

impl XWindowRecord {
    pub fn authority_surface(&self) -> AuthoritySurface {
        AuthoritySurface {
            authority: AuthorityKind::SophiaX,
            local_id: self.id.local,
            surface: self.surface,
            namespace: Some(self.namespace),
            mapped: self.map_state == XMapState::Mapped,
            geometry: self.geometry,
            constraints: self.constraints,
            generation: self.generation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XWindowLifecycleEvent {
    Created {
        id: XResourceId,
        surface: SurfaceId,
        namespace: NamespaceId,
        geometry: Rect,
        constraints: SurfaceConstraints,
        generation: u64,
    },
    Mapped {
        id: XResourceId,
        generation: u64,
    },
    Unmapped {
        id: XResourceId,
        generation: u64,
    },
    Configured {
        id: XResourceId,
        x: Option<i16>,
        y: Option<i16>,
        width: Option<u16>,
        height: Option<u16>,
        generation: u64,
    },
    Destroyed {
        id: XResourceId,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XWindowTable {
    windows: BTreeMap<XResourceId, XWindowRecord>,
}

impl XWindowTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(
        &mut self,
        event: XWindowLifecycleEvent,
    ) -> Result<Option<AuthoritySurface>, XAuthorityAccessError> {
        match event {
            XWindowLifecycleEvent::Created {
                id,
                surface,
                namespace,
                geometry,
                constraints,
                generation,
            } => {
                if !id.is_valid() {
                    return Err(XAuthorityAccessError::InvalidResource);
                }
                if !surface.is_valid() {
                    return Err(XAuthorityAccessError::InvalidResource);
                }
                if !namespace.is_valid() {
                    return Err(XAuthorityAccessError::InvalidNamespace);
                }

                let record = XWindowRecord {
                    id,
                    surface,
                    namespace,
                    map_state: XMapState::Unmapped,
                    geometry,
                    constraints,
                    generation,
                };
                let authority_surface = record.authority_surface();
                self.windows.insert(id, record);
                Ok(Some(authority_surface))
            }
            XWindowLifecycleEvent::Mapped { id, generation } => {
                let record = self
                    .windows
                    .get_mut(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?;
                record.map_state = XMapState::Mapped;
                record.generation = generation;
                Ok(Some(record.authority_surface()))
            }
            XWindowLifecycleEvent::Unmapped { id, generation } => {
                let record = self
                    .windows
                    .get_mut(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?;
                record.map_state = XMapState::Unmapped;
                record.generation = generation;
                Ok(Some(record.authority_surface()))
            }
            XWindowLifecycleEvent::Configured {
                id,
                x,
                y,
                width,
                height,
                generation,
            } => {
                let record = self
                    .windows
                    .get_mut(&id)
                    .ok_or(XAuthorityAccessError::UnknownResource)?;
                if let Some(x) = x {
                    record.geometry.x = i32::from(x);
                }
                if let Some(y) = y {
                    record.geometry.y = i32::from(y);
                }
                if let Some(width) = width {
                    record.geometry.width = i32::from(width);
                }
                if let Some(height) = height {
                    record.geometry.height = i32::from(height);
                }
                record.generation = generation;
                Ok(Some(record.authority_surface()))
            }
            XWindowLifecycleEvent::Destroyed { id } => {
                self.windows.remove(&id);
                Ok(None)
            }
        }
    }

    pub fn get(&self, id: XResourceId) -> Option<&XWindowRecord> {
        self.windows.get(&id)
    }

    pub fn advance_generation(
        &mut self,
        id: XResourceId,
        expected: u64,
    ) -> Result<u64, XAuthorityAccessError> {
        let record = self
            .windows
            .get_mut(&id)
            .ok_or(XAuthorityAccessError::UnknownResource)?;
        if record.generation != expected {
            return Err(XAuthorityAccessError::StaleGeneration);
        }
        let next = expected
            .checked_add(1)
            .ok_or(XAuthorityAccessError::InvalidResource)?;
        record.generation = next;
        Ok(next)
    }

    pub fn ids_for_namespace(&self, namespace: NamespaceId) -> Vec<XResourceId> {
        self.windows
            .values()
            .filter(|record| record.namespace == namespace)
            .map(|record| record.id)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}
