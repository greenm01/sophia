use sophia_protocol::{
    LayoutNodeCapabilities, LayoutNodeKind, LayoutNodeSnapshot, LayoutNodeState, Rect, Size,
    SurfaceConstraints, SurfaceId, WorkspaceId,
};

use super::error::WmProcessError;

pub(super) fn process_usage() -> &'static str {
    "usage: sophia-wm-demo relayout --transaction=N --output=N --workspace=N --bounds=x,y,w,h --node=index:generation[:x,y,w,h]"
}

pub(super) fn required_u64(args: &[String], key: &str) -> Result<u64, WmProcessError> {
    required_value(args, key)?
        .parse::<u64>()
        .map_err(|_| WmProcessError::new(format!("invalid {key} value")))
}

pub(super) fn required_rect(args: &[String], key: &str) -> Result<Rect, WmProcessError> {
    parse_rect(required_value(args, key)?)
}

pub(super) fn required_node(
    args: &[String],
    key: &str,
    workspace: WorkspaceId,
) -> Result<LayoutNodeSnapshot, WmProcessError> {
    parse_node(required_value(args, key)?, workspace)
}

pub(super) fn required_surface(args: &[String], key: &str) -> Result<SurfaceId, WmProcessError> {
    parse_surface_token(required_value(args, key)?)
}

pub(super) fn required_value<'a>(args: &'a [String], key: &str) -> Result<&'a str, WmProcessError> {
    arg_values(args, key)
        .into_iter()
        .next()
        .ok_or_else(|| WmProcessError::new(format!("missing {key}")))
}

pub(super) fn arg_values<'a>(args: &'a [String], key: &str) -> Vec<&'a str> {
    let prefix = format!("{key}=");
    args.iter()
        .filter_map(|arg| arg.strip_prefix(&prefix))
        .collect()
}

pub(super) fn response_u64(parts: &[&str], key: &str) -> Result<u64, WmProcessError> {
    response_value(parts, key)?
        .parse::<u64>()
        .map_err(|_| WmProcessError::new(format!("invalid response {key} value")))
}

pub(super) fn response_value<'a>(parts: &'a [&str], key: &str) -> Result<&'a str, WmProcessError> {
    let prefix = format!("{key}=");
    parts
        .iter()
        .find_map(|part| part.strip_prefix(&prefix))
        .ok_or_else(|| WmProcessError::new(format!("missing response {key}")))
}

pub(super) fn parse_node(
    value: &str,
    workspace: WorkspaceId,
) -> Result<LayoutNodeSnapshot, WmProcessError> {
    let fields = value.split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&fields.len()) {
        return Err(WmProcessError::new(format!("invalid node: {value}")));
    }
    let surface = parse_surface_pair(fields[0], fields[1])?;
    let geometry = if let Some(rect) = fields.get(2) {
        parse_rect(rect)?
    } else {
        Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        }
    };

    Ok(LayoutNodeSnapshot {
        surface,
        workspace,
        kind: LayoutNodeKind::Toplevel,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        state: LayoutNodeState::NORMAL,
        constraints: SurfaceConstraints {
            min_size: None,
            max_size: None,
        },
        geometry,
        generation: 1,
    })
}

pub(super) fn parse_surface_token(value: &str) -> Result<SurfaceId, WmProcessError> {
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != 2 {
        return Err(WmProcessError::new(format!("invalid surface: {value}")));
    }
    parse_surface_pair(fields[0], fields[1])
}

pub(super) fn parse_surface_pair(
    index: &str,
    generation: &str,
) -> Result<SurfaceId, WmProcessError> {
    let index = index
        .parse::<u32>()
        .map_err(|_| WmProcessError::new(format!("invalid surface index: {index}")))?;
    let generation = generation
        .parse::<u32>()
        .map_err(|_| WmProcessError::new(format!("invalid surface generation: {generation}")))?;
    Ok(SurfaceId::new(index, generation))
}

pub(super) fn parse_size(value: &str) -> Result<Size, WmProcessError> {
    let fields = value.split(',').collect::<Vec<_>>();
    if fields.len() != 2 {
        return Err(WmProcessError::new(format!("invalid size: {value}")));
    }
    Ok(Size {
        width: parse_i32(fields[0])?,
        height: parse_i32(fields[1])?,
    })
}

pub(super) fn parse_rect(value: &str) -> Result<Rect, WmProcessError> {
    let fields = value.split(',').collect::<Vec<_>>();
    if fields.len() != 4 {
        return Err(WmProcessError::new(format!("invalid rect: {value}")));
    }
    Ok(Rect {
        x: parse_i32(fields[0])?,
        y: parse_i32(fields[1])?,
        width: parse_i32(fields[2])?,
        height: parse_i32(fields[3])?,
    })
}

fn parse_i32(value: &str) -> Result<i32, WmProcessError> {
    value
        .parse::<i32>()
        .map_err(|_| WmProcessError::new(format!("invalid integer: {value}")))
}

pub(super) fn encode_node(node: &LayoutNodeSnapshot) -> String {
    format!(
        "{}:{}:{}",
        node.surface.index(),
        node.surface.generation(),
        encode_rect(node.geometry)
    )
}

pub(super) fn encode_rect(rect: Rect) -> String {
    format!("{},{},{},{}", rect.x, rect.y, rect.width, rect.height)
}

pub(super) fn encode_list(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(";")
    }
}
