use sophia_engine::{FocusedInputRoute, InputFocusDecision, InputFocusState};
use sophia_protocol::{
    BufferSource, CommittedSurfaceState, DeviceId, InputEventKind, InputEventPacket, Rect, Region,
    SeatId, SurfaceId,
};

fn committed(surface: SurfaceId) -> CommittedSurfaceState {
    CommittedSurfaceState {
        surface,
        committed_generation: 1,
        geometry: Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        },
        buffer: BufferSource::CpuBuffer { handle: 1 },
        damage: Region::empty(),
    }
}

fn key(seat: SeatId) -> InputEventPacket {
    InputEventPacket {
        serial: 1,
        seat,
        device: DeviceId::from_raw(1),
        time_msec: 2,
        kind: InputEventKind::Key {
            keycode: 30,
            pressed: true,
        },
        global_position: None,
        target_surface: None,
        target_window: None,
        local_position: None,
    }
}

#[test]
fn engine_focus_routes_keyboard_to_a_committed_surface() {
    let seat = SeatId::from_raw(1);
    let surface = SurfaceId::new(7, 1);
    let committed = vec![committed(surface)];
    let mut focus = InputFocusState::new();

    assert_eq!(
        focus.focus_surface(seat, surface, &committed),
        InputFocusDecision::Focused
    );
    let FocusedInputRoute::Routed(event) = focus.route_keyboard_event(key(seat), &committed) else {
        panic!("focused keyboard event should route");
    };
    assert_eq!(event.target_surface, Some(surface));
    assert_eq!(event.target_window, None);
}

#[test]
fn engine_focus_rejects_unknown_and_stale_surfaces() {
    let seat = SeatId::from_raw(1);
    let surface = SurfaceId::new(8, 1);
    let mut focus = InputFocusState::new();
    assert_eq!(
        focus.focus_surface(seat, surface, &[]),
        InputFocusDecision::UnknownSurface
    );

    assert_eq!(
        focus.focus_surface(seat, surface, &[committed(surface)]),
        InputFocusDecision::Focused
    );
    assert!(matches!(
        focus.route_keyboard_event(key(seat), &[]),
        FocusedInputRoute::StaleFocus(_)
    ));
    assert_eq!(focus.clear_surface(surface), 1);
    assert!(matches!(
        focus.route_keyboard_event(key(seat), &[]),
        FocusedInputRoute::NoFocus(_)
    ));
}
