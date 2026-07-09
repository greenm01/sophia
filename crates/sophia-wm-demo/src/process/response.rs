use sophia_protocol::{
    SurfacePlacement, SurfaceSizeRequest, TransactionId, Transform, WmCommand, WmResponsePacket,
    WorkspaceId,
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
        }
    }

    format!(
        "ok tx={} timeout={} assign={} configure={} focus={} render={}",
        response.transaction.raw(),
        response.timeout_msec,
        encode_list(&assign),
        encode_list(&configure),
        focus,
        encode_list(&render)
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

    Ok(WmResponsePacket {
        transaction,
        commands,
        timeout_msec,
    })
}
