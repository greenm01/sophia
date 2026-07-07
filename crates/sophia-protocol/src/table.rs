use crate::ids::SurfaceId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TableError {
    StaleId,
    InvalidId,
}

#[derive(Clone, Debug)]
struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

#[derive(Clone, Debug)]
pub struct SurfaceTable<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
}

impl<T> SurfaceTable<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn insert(&mut self, value: T) -> SurfaceId {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.generation = slot.generation.wrapping_add(1).max(1);
            slot.value = Some(value);
            SurfaceId::new(index, slot.generation)
        } else {
            let index = u32::try_from(self.slots.len()).expect("surface table overflow");
            self.slots.push(Slot {
                generation: 1,
                value: Some(value),
            });
            SurfaceId::new(index, 1)
        }
    }

    pub fn get(&self, id: SurfaceId) -> Option<&T> {
        self.slot(id).ok().and_then(|slot| slot.value.as_ref())
    }

    pub fn get_mut(&mut self, id: SurfaceId) -> Option<&mut T> {
        self.slot_mut(id).ok().and_then(|slot| slot.value.as_mut())
    }

    pub fn remove(&mut self, id: SurfaceId) -> Result<T, TableError> {
        let slot = self.slot_mut(id)?;
        let value = slot.value.take().ok_or(TableError::StaleId)?;
        self.free.push(id.index());
        Ok(value)
    }

    fn slot(&self, id: SurfaceId) -> Result<&Slot<T>, TableError> {
        if !id.is_valid() {
            return Err(TableError::InvalidId);
        }
        let slot = self
            .slots
            .get(id.index() as usize)
            .ok_or(TableError::InvalidId)?;
        if slot.generation != id.generation() {
            return Err(TableError::StaleId);
        }
        Ok(slot)
    }

    fn slot_mut(&mut self, id: SurfaceId) -> Result<&mut Slot<T>, TableError> {
        if !id.is_valid() {
            return Err(TableError::InvalidId);
        }
        let slot = self
            .slots
            .get_mut(id.index() as usize)
            .ok_or(TableError::InvalidId)?;
        if slot.generation != id.generation() {
            return Err(TableError::StaleId);
        }
        Ok(slot)
    }
}

impl<T> Default for SurfaceTable<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_surface_id_fails_closed() {
        let mut table = SurfaceTable::new();
        let first = table.insert("first");

        assert_eq!(table.remove(first), Ok("first"));

        let second = table.insert("second");

        assert_ne!(first, second);
        assert_eq!(table.get(first), None);
        assert_eq!(table.get(second), Some(&"second"));
    }
}
