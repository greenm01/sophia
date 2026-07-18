use std::collections::{BTreeMap, BTreeSet};

use sophia_protocol::{
    LayoutTransaction, OutputId, Rect, SurfaceId, SurfacePlacement, SurfaceSizeRequest,
    TransactionId, WM_API_VERSION, WmCommand, WmOutputWorkspace, WmResponsePacket, WmSessionAction,
    WmSessionDescriptor, WorkspaceId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WmOutputPolicyState {
    pub bounds: Rect,
    pub workspace: WorkspaceId,
    pub focus: Option<SurfaceId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmWorkspaceState {
    workspaces: Vec<WorkspaceId>,
    outputs: BTreeMap<OutputId, WmOutputPolicyState>,
    surfaces: BTreeMap<SurfaceId, WorkspaceId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WmPolicyError {
    InvalidWorkspaceCount,
    InvalidOutput,
    DuplicateOutput,
    InvalidOutputBounds,
    UnknownOutput,
    UnknownWorkspace,
    UnknownSurface,
    DuplicateSurfaceCommand,
    DuplicateOutputCommand,
    DuplicateFocusCommand,
    DuplicateSessionAction,
    HiddenFocus,
    UnadvertisedSessionAction,
    InvalidSessionActionTarget,
}

impl core::fmt::Display for WmPolicyError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for WmPolicyError {}

#[derive(Clone, Debug, PartialEq)]
pub struct WmPolicyPlan {
    pub transaction: TransactionId,
    pub candidate: WmWorkspaceState,
    pub layout: LayoutTransaction,
    pub affected_outputs: Vec<OutputId>,
    pub session_action: Option<(WmSessionAction, Option<SurfaceId>)>,
}

impl WmWorkspaceState {
    pub fn new(
        outputs: impl IntoIterator<Item = (OutputId, Rect)>,
        workspace_count: usize,
    ) -> Result<Self, WmPolicyError> {
        if workspace_count == 0 || workspace_count > 64 {
            return Err(WmPolicyError::InvalidWorkspaceCount);
        }
        let workspaces = (1..=workspace_count)
            .map(|slot| WorkspaceId::from_raw(slot as u64))
            .collect::<Vec<_>>();
        let mut configured = BTreeMap::new();
        for (index, (output, bounds)) in outputs.into_iter().enumerate() {
            if !output.is_valid() {
                return Err(WmPolicyError::InvalidOutput);
            }
            if bounds.is_empty() {
                return Err(WmPolicyError::InvalidOutputBounds);
            }
            let Some(workspace) = workspaces.get(index).copied() else {
                return Err(WmPolicyError::InvalidWorkspaceCount);
            };
            if configured
                .insert(
                    output,
                    WmOutputPolicyState {
                        bounds,
                        workspace,
                        focus: None,
                    },
                )
                .is_some()
            {
                return Err(WmPolicyError::DuplicateOutput);
            }
        }
        if configured.is_empty() {
            return Err(WmPolicyError::InvalidOutput);
        }
        Ok(Self {
            workspaces,
            outputs: configured,
            surfaces: BTreeMap::new(),
        })
    }

    pub fn descriptor(&self, session_actions: Vec<WmSessionAction>) -> WmSessionDescriptor {
        WmSessionDescriptor {
            api_version: WM_API_VERSION,
            workspaces: self.workspaces.clone(),
            active_workspaces: self
                .outputs
                .iter()
                .map(|(output, state)| WmOutputWorkspace {
                    output: *output,
                    workspace: state.workspace,
                })
                .collect(),
            session_actions,
        }
    }

    pub fn register_surface(
        &mut self,
        surface: SurfaceId,
        workspace: WorkspaceId,
    ) -> Result<(), WmPolicyError> {
        if !surface.is_valid() {
            return Err(WmPolicyError::UnknownSurface);
        }
        self.require_workspace(workspace)?;
        self.surfaces.insert(surface, workspace);
        Ok(())
    }

    pub fn remove_surface(&mut self, surface: SurfaceId) -> bool {
        let removed = self.surfaces.remove(&surface).is_some();
        for output in self.outputs.values_mut() {
            if output.focus == Some(surface) {
                output.focus = None;
            }
        }
        removed
    }

    pub fn surface_workspace(&self, surface: SurfaceId) -> Option<WorkspaceId> {
        self.surfaces.get(&surface).copied()
    }

    pub fn output(&self, output: OutputId) -> Option<WmOutputPolicyState> {
        self.outputs.get(&output).copied()
    }

    pub fn output_for_workspace(&self, workspace: WorkspaceId) -> Option<OutputId> {
        self.outputs
            .iter()
            .find_map(|(output, state)| (state.workspace == workspace).then_some(*output))
    }

    pub fn visible_surfaces(&self, output: OutputId) -> Result<Vec<SurfaceId>, WmPolicyError> {
        let workspace = self
            .outputs
            .get(&output)
            .ok_or(WmPolicyError::UnknownOutput)?
            .workspace;
        Ok(self
            .surfaces
            .iter()
            .filter_map(|(surface, assigned)| (*assigned == workspace).then_some(*surface))
            .collect())
    }

    pub fn plan_response(
        &self,
        response: &WmResponsePacket,
        advertised_actions: &[WmSessionAction],
    ) -> Result<WmPolicyPlan, WmPolicyError> {
        let mut candidate = self.clone();
        let mut requested_sizes = Vec::<SurfaceSizeRequest>::new();
        let mut render_positions = Vec::<SurfacePlacement>::new();
        let mut focus = None;
        let mut session_action = None;
        let mut assigned = BTreeSet::new();
        let mut activated = BTreeSet::new();
        let mut configured = BTreeSet::new();
        let mut rendered = BTreeSet::new();

        for command in &response.commands {
            match *command {
                WmCommand::ConfigureSurface(request) => {
                    candidate.require_surface(request.surface)?;
                    if !configured.insert(request.surface) {
                        return Err(WmPolicyError::DuplicateSurfaceCommand);
                    }
                    requested_sizes.push(request);
                }
                WmCommand::RenderSurface(placement) => {
                    candidate.require_surface(placement.surface)?;
                    if !rendered.insert(placement.surface) {
                        return Err(WmPolicyError::DuplicateSurfaceCommand);
                    }
                    render_positions.push(placement);
                }
                WmCommand::AssignWorkspace { surface, workspace } => {
                    candidate.require_surface(surface)?;
                    candidate.require_workspace(workspace)?;
                    if !assigned.insert(surface) {
                        return Err(WmPolicyError::DuplicateSurfaceCommand);
                    }
                    candidate.surfaces.insert(surface, workspace);
                }
                WmCommand::ActivateWorkspace { output, workspace } => {
                    candidate.require_workspace(workspace)?;
                    if !activated.insert(output) {
                        return Err(WmPolicyError::DuplicateOutputCommand);
                    }
                    candidate.activate_workspace(output, workspace)?;
                }
                WmCommand::FocusSurface(surface) => {
                    candidate.require_surface(surface)?;
                    if focus.replace(surface).is_some() {
                        return Err(WmPolicyError::DuplicateFocusCommand);
                    }
                }
                WmCommand::RequestSessionAction { action, target } => {
                    if session_action.is_some() {
                        return Err(WmPolicyError::DuplicateSessionAction);
                    }
                    if !advertised_actions.contains(&action) {
                        return Err(WmPolicyError::UnadvertisedSessionAction);
                    }
                    if let Some(surface) = target {
                        candidate.require_surface(surface)?;
                    }
                    if action != WmSessionAction::CloseFocused && target.is_some() {
                        return Err(WmPolicyError::InvalidSessionActionTarget);
                    }
                    session_action = Some((action, target));
                }
            }
        }

        if let Some(surface) = focus {
            let workspace = candidate
                .surface_workspace(surface)
                .ok_or(WmPolicyError::UnknownSurface)?;
            let output = candidate
                .output_for_workspace(workspace)
                .ok_or(WmPolicyError::HiddenFocus)?;
            candidate
                .outputs
                .get_mut(&output)
                .expect("visible workspace output exists")
                .focus = Some(surface);
        }
        candidate.clear_hidden_focus();

        let affected_outputs = candidate
            .outputs
            .iter()
            .filter_map(|(output, state)| {
                (self.outputs.get(output) != Some(state)).then_some(*output)
            })
            .collect();

        Ok(WmPolicyPlan {
            transaction: response.transaction,
            candidate,
            layout: LayoutTransaction {
                transaction: response.transaction,
                requested_sizes,
                focus,
                render_positions,
                timeout_msec: response.timeout_msec,
            },
            affected_outputs,
            session_action,
        })
    }

    fn activate_workspace(
        &mut self,
        output: OutputId,
        workspace: WorkspaceId,
    ) -> Result<(), WmPolicyError> {
        let previous = self
            .outputs
            .get(&output)
            .ok_or(WmPolicyError::UnknownOutput)?
            .workspace;
        if previous == workspace {
            return Ok(());
        }
        if let Some(other) = self.output_for_workspace(workspace) {
            self.outputs
                .get_mut(&other)
                .expect("workspace owner exists")
                .workspace = previous;
        }
        self.outputs
            .get_mut(&output)
            .expect("requested output exists")
            .workspace = workspace;
        Ok(())
    }

    fn clear_hidden_focus(&mut self) {
        let surface_workspaces = &self.surfaces;
        for output in self.outputs.values_mut() {
            if output.focus.is_some_and(|surface| {
                surface_workspaces.get(&surface).copied() != Some(output.workspace)
            }) {
                output.focus = None;
            }
        }
    }

    fn require_workspace(&self, workspace: WorkspaceId) -> Result<(), WmPolicyError> {
        if self.workspaces.contains(&workspace) {
            Ok(())
        } else {
            Err(WmPolicyError::UnknownWorkspace)
        }
    }

    fn require_surface(&self, surface: SurfaceId) -> Result<(), WmPolicyError> {
        if self.surfaces.contains_key(&surface) {
            Ok(())
        } else {
            Err(WmPolicyError::UnknownSurface)
        }
    }
}
