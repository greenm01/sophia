use crate::prelude::*;
use crate::state::*;

use super::wire::{SophiaRoutedInputDispatch, send_sophia_routed_input_route};

pub(super) struct RoutedInputHarness {
    pub(super) connection: x11rb::rust_connection::RustConnection,
    pub(super) routed_major_opcode: u8,
    pub(super) target: Window,
    pub(super) device: DeviceId,
}

impl RoutedInputHarness {
    pub(super) fn new(display_name: Option<&str>) -> Result<Self, XBridgeError> {
        let (connection, screen_num) =
            x11rb::connect(display_name).map_err(|error| XBridgeError::Connect {
                message: error.to_string(),
            })?;
        let routed_info = connection
            .extension_information(XLIBRE_ROUTED_INPUT_EXTENSION_NAME)
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .ok_or_else(|| XBridgeError::RoutedInput {
                message: format!("missing {XLIBRE_ROUTED_INPUT_EXTENSION_NAME} extension"),
            })?;
        let screen = connection
            .setup()
            .roots
            .get(screen_num)
            .ok_or(XBridgeError::InvalidScreen { screen_num })?;
        let device = master_pointer_device(&connection)?;
        let target = connection
            .generate_id()
            .map_err(|error| XBridgeError::GenerateId {
                message: error.to_string(),
            })?;
        let gc = connection
            .generate_id()
            .map_err(|error| XBridgeError::GenerateId {
                message: error.to_string(),
            })?;
        let target_width = 160;
        let target_height = 120;

        connection
            .create_window(
                screen.root_depth,
                target,
                screen.root,
                12,
                14,
                target_width,
                target_height,
                0,
                WindowClass::INPUT_OUTPUT,
                screen.root_visual,
                &CreateWindowAux::new()
                    .background_pixel(screen.white_pixel)
                    .event_mask(
                        EventMask::EXPOSURE
                            | EventMask::STRUCTURE_NOTIFY
                            | EventMask::BUTTON_PRESS
                            | EventMask::BUTTON_RELEASE
                            | EventMask::POINTER_MOTION,
                    ),
            )
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;
        connection
            .create_gc(
                gc,
                target,
                &CreateGCAux::new()
                    .foreground(screen.black_pixel)
                    .background(screen.white_pixel),
            )
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;
        connection
            .map_window(target)
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;
        connection
            .poly_fill_rectangle(
                target,
                gc,
                &[Rectangle {
                    x: 8,
                    y: 8,
                    width: target_width.saturating_sub(16),
                    height: target_height.saturating_sub(16),
                }],
            )
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .check()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;
        connection
            .flush()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;

        wait_for_mapped_window(&connection, target, Duration::from_secs(2))?;

        Ok(Self {
            connection,
            routed_major_opcode: routed_info.major_opcode,
            target,
            device,
        })
    }

    pub(super) fn target_window(&self) -> XWindowId {
        XWindowId::new(self.target, 1)
    }

    pub(super) fn send(
        &self,
        request: &XLibreRoutedInputRequest,
    ) -> Result<SophiaRoutedInputDispatch, XBridgeError> {
        send_sophia_routed_input_route(&self.connection, self.routed_major_opcode, request)
    }
}

fn master_pointer_device<C>(connection: &C) -> Result<DeviceId, XBridgeError>
where
    C: RequestConnection + ?Sized,
{
    let reply = connection
        .xinput_xi_query_device(Device::ALL_MASTER)
        .map_err(|error| XBridgeError::RoutedInput {
            message: error.to_string(),
        })?
        .reply()
        .map_err(|error| XBridgeError::RoutedInput {
            message: error.to_string(),
        })?;

    reply
        .infos
        .iter()
        .find(|info: &&XIDeviceInfo| info.enabled && info.type_ == DeviceType::MASTER_POINTER)
        .map(|info| DeviceId::from_raw(u64::from(info.deviceid)))
        .ok_or_else(|| XBridgeError::RoutedInput {
            message: "no enabled XInput master pointer found".to_owned(),
        })
}

pub(crate) fn drain_pending_events<C>(connection: &C) -> Result<(), XBridgeError>
where
    C: Connection + ?Sized,
{
    while connection
        .poll_for_event()
        .map_err(|error| XBridgeError::RoutedInput {
            message: error.to_string(),
        })?
        .is_some()
    {}

    Ok(())
}

fn wait_for_mapped_window<C>(
    connection: &C,
    window: Window,
    timeout: Duration,
) -> Result<(), XBridgeError>
where
    C: RequestConnection + ?Sized,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        let attrs = connection
            .get_window_attributes(window)
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?
            .reply()
            .map_err(|error| XBridgeError::RoutedInput {
                message: error.to_string(),
            })?;
        if attrs.map_state == MapState::VIEWABLE {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(XBridgeError::RoutedInput {
        message: format!("timed out waiting for routed-input target {window:#x} to map"),
    })
}

pub(super) fn wait_for_routed_button_press<C>(
    connection: &C,
    window: Window,
    timeout: Duration,
) -> Result<(i16, i16, u8), XBridgeError>
where
    C: Connection + ?Sized,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(event) =
            connection
                .poll_for_event()
                .map_err(|error| XBridgeError::RoutedInput {
                    message: error.to_string(),
                })?
        {
            if let Event::ButtonPress(event) = event {
                if event.event == window {
                    return Ok((event.event_x, event.event_y, event.detail));
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(XBridgeError::RoutedInput {
        message: format!("timed out waiting for routed button event on {window:#x}"),
    })
}
