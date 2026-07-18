use sophia_protocol::{
    OutputId, SurfacePlacement, SurfaceSizeRequest, TransactionId, Transform, WmCommand,
    WmResponsePacket, WmSessionAction, WorkspaceId,
};

use super::{
    codec::{
        encode_list, encode_rect, parse_rect, parse_size, parse_surface_pair, parse_surface_token,
        response_u64, response_value,
    },
    error::WmProcessError,
};

pub fn encode_process_response(response: &WmResponsePacket) -> String {
    let mut assign = Vec::new();
    let mut configure = Vec::new();
    let mut focus = String::from("-");
    let mut render = Vec::new();
    let mut activate = Vec::new();
    let mut session = Vec::new();

    for command in &response.commands {
        match command {
            WmCommand::AssignWorkspace { surface, workspace } => assign.push(format!(
                "{}:{}:{}",
                surface.index(),
                surface.generation(),
                workspace.raw()
            )),
            WmCommand::ConfigureSurface(request) => configure.push(format!(
                "{}:{}:{},{}",
                request.surface.index(),
                request.surface.generation(),
                request.size.width,
                request.size.height
            )),
            WmCommand::FocusSurface(surface) => {
                focus = format!("{}:{}", surface.index(), surface.generation());
            }
            WmCommand::RenderSurface(placement) => render.push(format!(
                "{}:{}:{}:{}:{}",
                placement.surface.index(),
                placement.surface.generation(),
                encode_rect(placement.geometry),
                placement.z_index,
                placement.crop.map_or_else(|| "-".to_owned(), encode_rect)
            )),
            WmCommand::ActivateWorkspace { output, workspace } => {
                activate.push(format!("{}:{}", output.raw(), workspace.raw()));
            }
            WmCommand::RequestSessionAction { action, target } => {
                let target = target.map_or_else(
                    || "-".to_owned(),
                    |surface| format!("{}:{}", surface.index(), surface.generation()),
                );
                session.push(format!("{}:{}", encode_session_action(*action), target));
            }
        }
    }

    format!(
        "ok tx={} timeout={} assign={} configure={} focus={} render={} activate={} session={}",
        response.transaction.raw(),
        response.timeout_msec,
        encode_list(&assign),
        encode_list(&configure),
        focus,
        encode_list(&render),
        encode_list(&activate),
        encode_list(&session)
    )
}

pub fn decode_process_response(line: &str) -> Result<WmResponsePacket, WmProcessError> {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    if parts.first().copied() != Some("ok") {
        return Err(WmProcessError::new(format!(
            "external WM returned invalid response: {line}"
        )));
    }

    let transaction = TransactionId::from_raw(response_u64(&parts, "tx")?);
    let timeout_msec = u32::try_from(response_u64(&parts, "timeout")?)
        .map_err(|_| WmProcessError::new("timeout does not fit u32"))?;
    let mut commands = Vec::new();

    for encoded in response_value(&parts, "assign")?.split(';') {
        if encoded == "-" || encoded.is_empty() {
            continue;
        }
        let fields = encoded.split(':').collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(WmProcessError::new(format!(
                "invalid assign command: {encoded}"
            )));
        }
        let surface = parse_surface_pair(fields[0], fields[1])?;
        let workspace = WorkspaceId::from_raw(
            fields[2]
                .parse::<u64>()
                .map_err(|_| WmProcessError::new(format!("invalid workspace: {}", fields[2])))?,
        );
        commands.push(WmCommand::AssignWorkspace { surface, workspace });
    }

    for encoded in response_value(&parts, "configure")?.split(';') {
        if encoded == "-" || encoded.is_empty() {
            continue;
        }
        let fields = encoded.split(':').collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(WmProcessError::new(format!(
                "invalid configure command: {encoded}"
            )));
        }
        let surface = parse_surface_pair(fields[0], fields[1])?;
        let size = parse_size(fields[2])?;
        commands.push(WmCommand::ConfigureSurface(SurfaceSizeRequest {
            surface,
            size,
        }));
    }

    let focus = response_value(&parts, "focus")?;
    if focus != "-" {
        commands.push(WmCommand::FocusSurface(parse_surface_token(focus)?));
    }

    for encoded in response_value(&parts, "render")?.split(';') {
        if encoded == "-" || encoded.is_empty() {
            continue;
        }
        let fields = encoded.split(':').collect::<Vec<_>>();
        if fields.len() != 5 {
            return Err(WmProcessError::new(format!(
                "invalid render command: {encoded}"
            )));
        }
        let surface = parse_surface_pair(fields[0], fields[1])?;
        let geometry = parse_rect(fields[2])?;
        let z_index = fields[3]
            .parse::<i32>()
            .map_err(|_| WmProcessError::new(format!("invalid z-index: {}", fields[3])))?;
        let crop = if fields[4] == "-" {
            None
        } else {
            Some(parse_rect(fields[4])?)
        };
        commands.push(WmCommand::RenderSurface(SurfacePlacement {
            surface,
            geometry,
            z_index,
            crop,
            transform: Transform::IDENTITY,
        }));
    }
    for encoded in response_value(&parts, "activate")?.split(';') {
        if encoded == "-" || encoded.is_empty() {
            continue;
        }
        let fields = encoded.split(':').collect::<Vec<_>>();
        if fields.len() != 2 {
            return Err(WmProcessError::new(format!(
                "invalid activate command: {encoded}"
            )));
        }
        let output = OutputId::from_raw(parse_u64_field(fields[0], "output")?);
        let workspace = WorkspaceId::from_raw(parse_u64_field(fields[1], "workspace")?);
        commands.push(WmCommand::ActivateWorkspace { output, workspace });
    }

    for encoded in response_value(&parts, "session")?.split(';') {
        if encoded == "-" || encoded.is_empty() {
            continue;
        }
        let mut fields = encoded.splitn(2, ':');
        let action = decode_session_action(
            fields
                .next()
                .ok_or_else(|| WmProcessError::new("missing session action"))?,
        )?;
        let target = match fields
            .next()
            .ok_or_else(|| WmProcessError::new("missing session target"))?
        {
            "-" => None,
            target => Some(parse_surface_token(target)?),
        };
        commands.push(WmCommand::RequestSessionAction { action, target });
    }

    Ok(WmResponsePacket {
        transaction,
        commands,
        timeout_msec,
    })
}
fn encode_session_action(action: WmSessionAction) -> &'static str {
    match action {
        WmSessionAction::LaunchTerminal => "terminal",
        WmSessionAction::LaunchApplicationMenu => "launcher",
        WmSessionAction::LaunchFirefox => "firefox",
        WmSessionAction::CloseFocused => "close",
        WmSessionAction::Logout => "logout",
    }
}

fn decode_session_action(value: &str) -> Result<WmSessionAction, WmProcessError> {
    match value {
        "terminal" => Ok(WmSessionAction::LaunchTerminal),
        "launcher" => Ok(WmSessionAction::LaunchApplicationMenu),
        "firefox" => Ok(WmSessionAction::LaunchFirefox),
        "close" => Ok(WmSessionAction::CloseFocused),
        "logout" => Ok(WmSessionAction::Logout),
        _ => Err(WmProcessError::new(format!(
            "invalid session action: {value}"
        ))),
    }
}

fn parse_u64_field(value: &str, field: &str) -> Result<u64, WmProcessError> {
    value
        .parse()
        .map_err(|_| WmProcessError::new(format!("invalid {field}: {value}")))
}
