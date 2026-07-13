use std::collections::BTreeMap;

use sophia_protocol::{
    AuthorityFeedback, AuthorityKind, AuthorityLocalId, BufferReleaseFeedback, BufferSource,
    NamespaceId, Rect, Region, Size, SurfaceId, SurfacePresentationFeedback, SurfaceTransaction,
    SurfaceTransactionReadiness, TransactionCommit, TransactionId, TransactionOutcome,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaylandAuthorityError {
    InvalidNamespace,
    InvalidLocalId,
    InvalidSurface,
    InvalidGeometry,
    InvalidBuffer,
    UnknownSurface,
    DuplicateSurface,
    RoleAlreadyAssigned,
    InvalidConfigureSerial,
    OutOfOrderTransaction,
    UnknownTransaction,
    StalePresentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaylandSurfaceRole {
    Toplevel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaylandSurfaceEvent {
    Created {
        namespace: NamespaceId,
        local_id: AuthorityLocalId,
        surface: SurfaceId,
        geometry: Rect,
    },
    AssignRole {
        local_id: AuthorityLocalId,
        role: WaylandSurfaceRole,
    },
    Attach {
        local_id: AuthorityLocalId,
        buffer: BufferSource,
    },
    Detach {
        local_id: AuthorityLocalId,
    },
    Damage {
        local_id: AuthorityLocalId,
        damage: Region,
    },
    SetGeometry {
        local_id: AuthorityLocalId,
        geometry: Rect,
    },
    RequestFrame {
        local_id: AuthorityLocalId,
        callback: u64,
    },
    Commit {
        local_id: AuthorityLocalId,
        transaction: TransactionId,
        timeout_msec: u32,
    },
    Destroyed {
        local_id: AuthorityLocalId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaylandXdgEvent {
    Configure {
        local_id: AuthorityLocalId,
        serial: u32,
        size: Size,
    },
    AckConfigure {
        local_id: AuthorityLocalId,
        serial: u32,
    },
    Close {
        local_id: AuthorityLocalId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaylandAuthorityAction {
    None,
    SurfaceTransaction(SurfaceTransaction),
    TransactionAccepted {
        transaction: TransactionId,
        surface: SurfaceId,
        generation: u64,
    },
    TransactionRejected(TransactionCommit),
    FrameDone {
        callback: u64,
        presentation_msec: u64,
    },
    BufferReleased(BufferReleaseFeedback),
    CloseRequested {
        surface: SurfaceId,
    },
    SurfaceDestroyed {
        surface: SurfaceId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PendingBuffer {
    Attach(BufferSource),
    Detach,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingCommit {
    local_id: AuthorityLocalId,
    transaction: SurfaceTransaction,
    callbacks: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PresentedBuffer {
    generation: u64,
    source: BufferSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WaylandSurfaceState {
    namespace: NamespaceId,
    surface: SurfaceId,
    role: Option<WaylandSurfaceRole>,
    geometry: Rect,
    committed_buffer: BufferSource,
    committed_generation: u64,
    pending_buffer: Option<PendingBuffer>,
    pending_damage: Region,
    pending_geometry: Option<Rect>,
    pending_callbacks: Vec<u64>,
    committed_callbacks: BTreeMap<u64, Vec<u64>>,
    committed_buffers: BTreeMap<u64, BufferSource>,
    required_configure: Option<u32>,
    acked_configure: Option<u32>,
    presented: Option<PresentedBuffer>,
    in_flight: Vec<TransactionId>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WaylandAuthorityReducer {
    surfaces: BTreeMap<AuthorityLocalId, WaylandSurfaceState>,
    surface_to_local: BTreeMap<SurfaceId, AuthorityLocalId>,
    pending_commits: BTreeMap<TransactionId, PendingCommit>,
}

impl WaylandAuthorityReducer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn surface_count(&self) -> usize {
        self.surfaces.len()
    }

    pub fn apply_surface_event(
        &mut self,
        event: WaylandSurfaceEvent,
    ) -> Result<Vec<WaylandAuthorityAction>, WaylandAuthorityError> {
        match event {
            WaylandSurfaceEvent::Created {
                namespace,
                local_id,
                surface,
                geometry,
            } => {
                validate_created(namespace, local_id, surface, geometry)?;
                if self.surfaces.contains_key(&local_id)
                    || self.surface_to_local.contains_key(&surface)
                {
                    return Err(WaylandAuthorityError::DuplicateSurface);
                }
                self.surfaces.insert(
                    local_id,
                    WaylandSurfaceState {
                        namespace,
                        surface,
                        role: None,
                        geometry,
                        committed_buffer: BufferSource::None,
                        committed_generation: 0,
                        pending_buffer: None,
                        pending_damage: Region::empty(),
                        pending_geometry: None,
                        pending_callbacks: Vec::new(),
                        committed_callbacks: BTreeMap::new(),
                        committed_buffers: BTreeMap::new(),
                        required_configure: None,
                        acked_configure: None,
                        presented: None,
                        in_flight: Vec::new(),
                    },
                );
                self.surface_to_local.insert(surface, local_id);
                Ok(vec![WaylandAuthorityAction::None])
            }
            WaylandSurfaceEvent::AssignRole { local_id, role } => {
                let state = self.surface_mut(local_id)?;
                if state.role.is_some() {
                    return Err(WaylandAuthorityError::RoleAlreadyAssigned);
                }
                state.role = Some(role);
                Ok(vec![WaylandAuthorityAction::None])
            }
            WaylandSurfaceEvent::Attach { local_id, buffer } => {
                if matches!(buffer, BufferSource::None) {
                    return Err(WaylandAuthorityError::InvalidBuffer);
                }
                self.surface_mut(local_id)?.pending_buffer = Some(PendingBuffer::Attach(buffer));
                Ok(vec![WaylandAuthorityAction::None])
            }
            WaylandSurfaceEvent::Detach { local_id } => {
                self.surface_mut(local_id)?.pending_buffer = Some(PendingBuffer::Detach);
                Ok(vec![WaylandAuthorityAction::None])
            }
            WaylandSurfaceEvent::Damage { local_id, damage } => {
                self.surface_mut(local_id)?.pending_damage.extend(&damage);
                Ok(vec![WaylandAuthorityAction::None])
            }
            WaylandSurfaceEvent::SetGeometry { local_id, geometry } => {
                if geometry.is_empty() {
                    return Err(WaylandAuthorityError::InvalidGeometry);
                }
                self.surface_mut(local_id)?.pending_geometry = Some(geometry);
                Ok(vec![WaylandAuthorityAction::None])
            }
            WaylandSurfaceEvent::RequestFrame { local_id, callback } => {
                if callback == 0 {
                    return Err(WaylandAuthorityError::InvalidConfigureSerial);
                }
                self.surface_mut(local_id)?.pending_callbacks.push(callback);
                Ok(vec![WaylandAuthorityAction::None])
            }
            WaylandSurfaceEvent::Commit {
                local_id,
                transaction,
                timeout_msec,
            } => self.commit(local_id, transaction, timeout_msec),
            WaylandSurfaceEvent::Destroyed { local_id } => self.destroy(local_id),
        }
    }

    pub fn apply_xdg_event(
        &mut self,
        event: WaylandXdgEvent,
    ) -> Result<Vec<WaylandAuthorityAction>, WaylandAuthorityError> {
        match event {
            WaylandXdgEvent::Configure {
                local_id,
                serial,
                size,
            } => {
                if serial == 0 || size.width <= 0 || size.height <= 0 {
                    return Err(WaylandAuthorityError::InvalidConfigureSerial);
                }
                let state = self.surface_mut(local_id)?;
                state.required_configure = Some(serial);
                state.pending_geometry = Some(Rect {
                    x: state.geometry.x,
                    y: state.geometry.y,
                    width: size.width,
                    height: size.height,
                });
                Ok(vec![WaylandAuthorityAction::None])
            }
            WaylandXdgEvent::AckConfigure { local_id, serial } => {
                let state = self.surface_mut(local_id)?;
                let Some(required) = state.required_configure else {
                    return Err(WaylandAuthorityError::InvalidConfigureSerial);
                };
                if serial < required {
                    return Err(WaylandAuthorityError::InvalidConfigureSerial);
                }
                state.acked_configure = Some(serial);
                Ok(vec![WaylandAuthorityAction::None])
            }
            WaylandXdgEvent::Close { local_id } => {
                let surface = self.surface_mut(local_id)?.surface;
                Ok(vec![WaylandAuthorityAction::CloseRequested { surface }])
            }
        }
    }

    pub fn apply_feedback(
        &mut self,
        feedback: AuthorityFeedback,
    ) -> Result<Vec<WaylandAuthorityAction>, WaylandAuthorityError> {
        match feedback {
            AuthorityFeedback::Transaction(commit) => self.apply_transaction_commit(commit),
            AuthorityFeedback::FrameScheduled(scheduled) => self.apply_frame_scheduled(scheduled),
            AuthorityFeedback::Presented(presented) => self.apply_presented(presented),
        }
    }

    fn commit(
        &mut self,
        local_id: AuthorityLocalId,
        transaction: TransactionId,
        timeout_msec: u32,
    ) -> Result<Vec<WaylandAuthorityAction>, WaylandAuthorityError> {
        if !transaction.is_valid() || self.pending_commits.contains_key(&transaction) {
            return Err(WaylandAuthorityError::UnknownTransaction);
        }
        let staged = self
            .surfaces
            .get(&local_id)
            .ok_or(WaylandAuthorityError::UnknownSurface)?
            .in_flight
            .last()
            .and_then(|transaction| self.pending_commits.get(transaction))
            .map(|pending| {
                (
                    pending.transaction.target_buffer,
                    pending.transaction.target_geometry,
                    pending
                        .transaction
                        .previous_committed_generation
                        .saturating_add(1),
                )
            });
        let state = self.surface_mut(local_id)?;
        let (base_buffer, base_geometry, previous_committed_generation) = staged.unwrap_or((
            state.committed_buffer,
            state.geometry,
            state.committed_generation,
        ));

        let target_buffer = match state.pending_buffer.as_ref() {
            Some(PendingBuffer::Attach(buffer)) => *buffer,
            Some(PendingBuffer::Detach) => BufferSource::None,
            None => base_buffer,
        };
        if matches!(target_buffer, BufferSource::None) && matches!(base_buffer, BufferSource::None)
        {
            state.pending_buffer = None;
            state.pending_damage = Region::empty();
            state.pending_geometry = None;
            state.pending_callbacks.clear();
            return Ok(vec![WaylandAuthorityAction::None]);
        }

        let configure_acked =
            state.required_configure.is_none() || state.acked_configure >= state.required_configure;
        let surface_transaction = SurfaceTransaction {
            transaction,
            authority: AuthorityKind::SophiaWayland,
            surface: state.surface,
            namespace: Some(state.namespace),
            target_geometry: if configure_acked {
                state.pending_geometry.unwrap_or(base_geometry)
            } else {
                base_geometry
            },
            target_buffer,
            damage: state.pending_damage.clone(),
            readiness: SurfaceTransactionReadiness::Ready,
            timeout_msec,
            previous_committed_generation,
        };
        let pending = PendingCommit {
            local_id,
            transaction: surface_transaction.clone(),
            callbacks: std::mem::take(&mut state.pending_callbacks),
        };
        state.pending_buffer = None;
        state.pending_damage = Region::empty();
        if configure_acked {
            state.pending_geometry = None;
        }
        state.in_flight.push(transaction);
        self.pending_commits.insert(transaction, pending);
        Ok(vec![WaylandAuthorityAction::SurfaceTransaction(
            surface_transaction,
        )])
    }

    fn apply_transaction_commit(
        &mut self,
        commit: TransactionCommit,
    ) -> Result<Vec<WaylandAuthorityAction>, WaylandAuthorityError> {
        let Some(pending) = self.pending_commits.remove(&commit.transaction) else {
            return Err(WaylandAuthorityError::UnknownTransaction);
        };
        // `wl_surface.destroy` can be handled in the same Wayland dispatch batch
        // as its final commit.  The frontend has already emitted that commit to
        // the engine, so let its completion drain quietly after destruction rather
        // than turning normal client teardown into a fatal protocol error.
        let Some(state) = self.surfaces.get_mut(&pending.local_id) else {
            return Ok(Vec::new());
        };
        if state.in_flight.first().copied() != Some(commit.transaction) {
            return Err(WaylandAuthorityError::OutOfOrderTransaction);
        }
        state.in_flight.remove(0);
        if commit.outcome != TransactionOutcome::Committed
            || !commit.applied_surfaces.contains(&state.surface)
        {
            return Ok(vec![WaylandAuthorityAction::TransactionRejected(commit)]);
        }

        state.geometry = pending.transaction.target_geometry;
        state.committed_buffer = pending.transaction.target_buffer;
        state.committed_generation = state.committed_generation.saturating_add(1);
        state
            .committed_buffers
            .insert(state.committed_generation, state.committed_buffer);
        state
            .committed_callbacks
            .insert(state.committed_generation, pending.callbacks);
        Ok(vec![WaylandAuthorityAction::TransactionAccepted {
            transaction: commit.transaction,
            surface: state.surface,
            generation: state.committed_generation,
        }])
    }

    fn apply_presented(
        &mut self,
        feedback: SurfacePresentationFeedback,
    ) -> Result<Vec<WaylandAuthorityAction>, WaylandAuthorityError> {
        let Some(local_id) = self.surface_to_local.get(&feedback.surface).copied() else {
            return Err(WaylandAuthorityError::UnknownSurface);
        };
        let state = self.surface_mut(local_id)?;
        if feedback.generation > state.committed_generation
            || state
                .presented
                .as_ref()
                .is_some_and(|presented| feedback.generation <= presented.generation)
        {
            return Err(WaylandAuthorityError::StalePresentation);
        }

        let source = state
            .committed_buffers
            .get(&feedback.generation)
            .copied()
            .ok_or(WaylandAuthorityError::StalePresentation)?;

        // A page flip may coalesce several client commits. Complete every frame
        // callback up to the frame that actually reached scanout and release all
        // superseded buffers, rather than retaining skipped generations forever.
        let completed_callback_generations = state
            .committed_callbacks
            .keys()
            .copied()
            .filter(|generation| *generation <= feedback.generation)
            .collect::<Vec<_>>();
        let mut actions = Vec::new();
        for generation in completed_callback_generations {
            for callback in state
                .committed_callbacks
                .remove(&generation)
                .unwrap_or_default()
            {
                actions.push(WaylandAuthorityAction::FrameDone {
                    callback,
                    presentation_msec: feedback.presentation_msec,
                });
            }
        }

        let superseded_sources = state
            .committed_buffers
            .iter()
            .filter(|(generation, candidate)| {
                **generation < feedback.generation
                    && **candidate != source
                    && !matches!(candidate, BufferSource::None)
            })
            .map(|(_, candidate)| *candidate)
            .collect::<Vec<_>>();
        state.presented = Some(PresentedBuffer {
            generation: feedback.generation,
            source,
        });
        state
            .committed_buffers
            .retain(|generation, _| *generation >= feedback.generation);
        for source in superseded_sources {
            if actions.iter().any(|action| {
                matches!(
                    action,
                    WaylandAuthorityAction::BufferReleased(release) if release.source == source
                )
            }) {
                continue;
            }
            actions.push(WaylandAuthorityAction::BufferReleased(
                BufferReleaseFeedback {
                    surface: state.surface,
                    source,
                },
            ));
        }
        Ok(actions)
    }

    fn apply_frame_scheduled(
        &mut self,
        feedback: SurfacePresentationFeedback,
    ) -> Result<Vec<WaylandAuthorityAction>, WaylandAuthorityError> {
        let Some(local_id) = self.surface_to_local.get(&feedback.surface).copied() else {
            return Err(WaylandAuthorityError::UnknownSurface);
        };
        let state = self.surface_mut(local_id)?;
        if feedback.generation == 0 || feedback.generation > state.committed_generation {
            return Err(WaylandAuthorityError::StalePresentation);
        }
        // A claimed composition lets the client prepare a subsequent frame.
        // Unlike `Presented`, it must not release the current client buffer
        // before the corresponding KMS page flip retires.
        let scheduled_generations = state
            .committed_callbacks
            .keys()
            .copied()
            .filter(|generation| *generation <= feedback.generation)
            .collect::<Vec<_>>();
        let mut actions = Vec::new();
        for generation in scheduled_generations {
            for callback in state
                .committed_callbacks
                .remove(&generation)
                .unwrap_or_default()
            {
                actions.push(WaylandAuthorityAction::FrameDone {
                    callback,
                    presentation_msec: feedback.presentation_msec,
                });
            }
        }
        Ok(actions)
    }

    fn destroy(
        &mut self,
        local_id: AuthorityLocalId,
    ) -> Result<Vec<WaylandAuthorityAction>, WaylandAuthorityError> {
        let Some(state) = self.surfaces.remove(&local_id) else {
            return Err(WaylandAuthorityError::UnknownSurface);
        };
        let mut buffers = state
            .committed_buffers
            .values()
            .copied()
            .collect::<Vec<_>>();
        if let Some(PendingBuffer::Attach(source)) = state.pending_buffer {
            buffers.push(source);
        }
        for transaction in state.in_flight {
            if let Some(pending) = self.pending_commits.get(&transaction) {
                buffers.push(pending.transaction.target_buffer);
            }
        }
        self.surface_to_local.remove(&state.surface);
        let mut actions = Vec::new();
        for source in buffers {
            if matches!(source, BufferSource::None)
                || actions.iter().any(|action| {
                    matches!(
                        action,
                        WaylandAuthorityAction::BufferReleased(release)
                            if release.source == source
                    )
                })
            {
                continue;
            }
            actions.push(WaylandAuthorityAction::BufferReleased(
                BufferReleaseFeedback {
                    surface: state.surface,
                    source,
                },
            ));
        }
        actions.push(WaylandAuthorityAction::SurfaceDestroyed {
            surface: state.surface,
        });
        Ok(actions)
    }

    fn surface_mut(
        &mut self,
        local_id: AuthorityLocalId,
    ) -> Result<&mut WaylandSurfaceState, WaylandAuthorityError> {
        self.surfaces
            .get_mut(&local_id)
            .ok_or(WaylandAuthorityError::UnknownSurface)
    }
}

fn validate_created(
    namespace: NamespaceId,
    local_id: AuthorityLocalId,
    surface: SurfaceId,
    geometry: Rect,
) -> Result<(), WaylandAuthorityError> {
    if !namespace.is_valid() {
        return Err(WaylandAuthorityError::InvalidNamespace);
    }
    if !local_id.is_valid() {
        return Err(WaylandAuthorityError::InvalidLocalId);
    }
    if !surface.is_valid() {
        return Err(WaylandAuthorityError::InvalidSurface);
    }
    if geometry.is_empty() {
        return Err(WaylandAuthorityError::InvalidGeometry);
    }
    Ok(())
}
