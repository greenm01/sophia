#![allow(dead_code, unused_imports)]

pub use sophia_portal::{ClipboardPortal, ClipboardTarget, PortalCommand, PortalError};
pub use sophia_protocol::*;
pub use sophia_x_bridge::*;
pub use x11rb::protocol::Event;
pub use x11rb::protocol::damage::ReportLevel;
pub use x11rb::protocol::xfixes::SelectionEvent;
pub use x11rb::protocol::xproto::SelectionRequestEvent;

pub fn xid(window: u32) -> XWindowId {
    XWindowId::new(window, 1)
}

pub fn status(extension: RequiredExtension, present: bool) -> ExtensionStatus {
    ExtensionStatus {
        extension,
        present,
        major_opcode: present.then_some(128),
        first_event: present.then_some(64),
        first_error: present.then_some(32),
    }
}

pub fn mirror(window: u32, parent: Option<u32>, stack_rank: u32) -> XWindowMirror {
    XWindowMirror {
        window: xid(window),
        parent: parent.map(xid),
        children: Vec::new(),
        toplevel: None,
        client: None,
        mapped: false,
        stack_rank,
        geometry: Rect {
            x: i32::try_from(window).unwrap_or(0),
            y: 0,
            width: 100,
            height: 50,
        },
        namespace: None,
        stale_metadata: 0,
    }
}

pub fn input_event(serial: u64) -> InputEventPacket {
    InputEventPacket {
        serial,
        seat: SeatId::from_raw(1),
        device: DeviceId::from_raw(2),
        time_msec: 1_000,
        kind: InputEventKind::PointerButton {
            button: 1,
            pressed: true,
        },
        global_position: Some(Point { x: 100.0, y: 200.0 }),
        target_surface: Some(SurfaceId::new(3, 1)),
        target_window: Some(xid(0x30)),
        local_position: Some(Point { x: 12.0, y: 8.0 }),
    }
}

pub fn input_route(
    serial: u64,
    outcome: InputRouteOutcome,
    target_window: Option<XWindowId>,
    local_position: Option<Point>,
    transform: Transform,
) -> InputRoute {
    InputRoute {
        input_serial: serial,
        target_surface: Some(SurfaceId::new(3, 1)),
        target_window,
        global_position: Point { x: 100.0, y: 200.0 },
        local_position,
        transform,
        outcome,
    }
}

pub fn selection_request(owner: u32, requestor: u32) -> SelectionRequestEvent {
    SelectionRequestEvent {
        response_type: 0,
        sequence: 1,
        time: 55,
        owner,
        requestor,
        selection: 0x100,
        target: 0x200,
        property: 0x300,
    }
}

pub fn selection_request_event(owner: u32, requestor: u32) -> Event {
    Event::SelectionRequest(selection_request(owner, requestor))
}

pub fn clipboard_portal_request(
    transfer: u64,
    property: u32,
    target: &str,
) -> ClipboardSelectionPortalRequest {
    ClipboardSelectionPortalRequest {
        request: sophia_portal::ClipboardTransferRequest {
            transfer: PortalTransferId::from_raw(transfer),
            source_namespace: NamespaceId::from_raw(7),
            target_namespace: NamespaceId::from_raw(9),
            target: ClipboardTarget::Atom(target.to_owned()),
            byte_size: 0,
            generation: 1,
        },
        failure: ClipboardSelectionFailureRequest {
            transfer: PortalTransferId::from_raw(transfer),
            requestor: 0x44,
            selection: 0x100,
            target: 0x200,
            time: 55,
        },
        property,
    }
}
