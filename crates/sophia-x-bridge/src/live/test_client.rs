use crate::prelude::*;
use crate::state::*;

pub fn run_test_client_window(config: TestClientConfig) -> Result<TestClientWindow, XBridgeError> {
    let width = u16::try_from(config.size.width.max(1)).unwrap_or(u16::MAX);
    let height = u16::try_from(config.size.height.max(1)).unwrap_or(u16::MAX);
    let (connection, screen_num) =
        x11rb::connect(config.display_name.as_deref()).map_err(|error| XBridgeError::Connect {
            message: error.to_string(),
        })?;
    let screen = connection
        .setup()
        .roots
        .get(screen_num)
        .ok_or(XBridgeError::InvalidScreen { screen_num })?;
    let window = connection
        .generate_id()
        .map_err(|error| XBridgeError::GenerateId {
            message: error.to_string(),
        })?;
    let gc = connection
        .generate_id()
        .map_err(|error| XBridgeError::GenerateId {
            message: error.to_string(),
        })?;
    let window_aux = CreateWindowAux::new()
        .background_pixel(screen.white_pixel)
        .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY);

    connection
        .create_window(
            screen.root_depth,
            window,
            screen.root,
            0,
            0,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &window_aux,
        )
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .create_gc(
            gc,
            window,
            &CreateGCAux::new()
                .foreground(screen.black_pixel)
                .background(screen.white_pixel),
        )
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .map_window(window)
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .poly_fill_rectangle(
            window,
            gc,
            &[Rectangle {
                x: 24,
                y: 24,
                width: width.saturating_sub(48),
                height: height.saturating_sub(48),
            }],
        )
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?
        .check()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;
    connection
        .flush()
        .map_err(|error| XBridgeError::TestClient {
            message: error.to_string(),
        })?;

    thread::sleep(Duration::from_millis(config.hold_millis));

    Ok(TestClientWindow {
        window: wrap_xid(window),
        size: Size {
            width: i32::from(width),
            height: i32::from(height),
        },
    })
}
