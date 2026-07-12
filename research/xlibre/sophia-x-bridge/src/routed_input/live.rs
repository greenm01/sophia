use crate::prelude::*;
use crate::state::*;

use super::adapter::routed_input_decision_allows_delivery;
use super::harness::{RoutedInputHarness, drain_pending_events, wait_for_routed_button_press};
use super::model::{RoutedInputDispatchStats, RoutedInputSmokeReport, RoutedInputStressReport};

pub fn smoke_routed_input(
    display_name: Option<&str>,
) -> Result<RoutedInputSmokeReport, XBridgeError> {
    let harness = RoutedInputHarness::new(display_name)?;
    let local_x = 42;
    let local_y = 37;
    let button = 1;
    let serial = 0x534f_5048_4941_0001;

    let request = XLibreRoutedInputRequest {
        serial,
        seat: SeatId::from_raw(1),
        device: harness.device,
        time_msec: 1,
        target_window: harness.target_window(),
        local_position: Point {
            x: f64::from(local_x),
            y: f64::from(local_y),
        },
        kind: InputEventKind::PointerButton {
            button: u32::from(button),
            pressed: true,
        },
    };
    let dispatch = harness.send(&request)?;
    let decision = XLibreRoutedInputDecision {
        serial: dispatch.reply.serial,
        target_window: dispatch.reply.target_window,
        outcome: dispatch.reply.outcome,
    };
    if !routed_input_decision_allows_delivery(&decision) {
        return Err(XBridgeError::RoutedInput {
            message: format!("routed input rejected with {:?}", decision.outcome),
        });
    }

    let (event_x, event_y, observed_button) =
        wait_for_routed_button_press(&harness.connection, harness.target, Duration::from_secs(2))?;

    if event_x != local_x || event_y != local_y || observed_button != button {
        return Err(XBridgeError::RoutedInput {
            message: format!(
                "unexpected routed button event local=({}, {}) button={}, expected=({}, {}) button={}",
                event_x, event_y, observed_button, local_x, local_y, button
            ),
        });
    }

    Ok(RoutedInputSmokeReport {
        display_name: display_name.map(str::to_owned),
        extension_opcode: harness.routed_major_opcode,
        target_window: harness.target_window(),
        device: harness.device,
        decision,
        dispatch_elapsed: dispatch.elapsed,
        request_bytes: dispatch.request_bytes,
        event_x,
        event_y,
        button: observed_button,
    })
}

pub fn stress_routed_input(
    display_name: Option<&str>,
    iterations: usize,
    threshold: Duration,
) -> Result<RoutedInputStressReport, XBridgeError> {
    if iterations == 0 {
        return Err(XBridgeError::RoutedInput {
            message: "routed-input stress iterations must be greater than zero".to_owned(),
        });
    }

    let harness = RoutedInputHarness::new(display_name)?;
    let mut stats = RoutedInputDispatchStats::new();
    let mut accepted = 0;
    let mut request_bytes = 0;
    let base_serial = 0x534f_5048_5354_0000;

    for index in 0..iterations {
        let local_x = 8 + i32::try_from(index % 120).unwrap_or(0);
        let local_y = 8 + i32::try_from((index / 120) % 80).unwrap_or(0);
        let request = XLibreRoutedInputRequest {
            serial: base_serial + u64::try_from(index).unwrap_or(u64::MAX),
            seat: SeatId::from_raw(1),
            device: harness.device,
            time_msec: u64::try_from(index).unwrap_or(u64::MAX),
            target_window: harness.target_window(),
            local_position: Point {
                x: f64::from(local_x),
                y: f64::from(local_y),
            },
            kind: InputEventKind::PointerMotion,
        };
        let dispatch = harness.send(&request)?;
        request_bytes = dispatch.request_bytes;
        stats.record(dispatch.elapsed);

        if dispatch.reply.outcome == XLibreRoutedInputOutcome::Accepted {
            accepted += 1;
        } else {
            return Err(XBridgeError::RoutedInput {
                message: format!(
                    "routed-input stress rejected sample {index} with {:?}",
                    dispatch.reply.outcome
                ),
            });
        }

        if index % 128 == 0 {
            drain_pending_events(&harness.connection)?;
        }
    }

    drain_pending_events(&harness.connection)?;
    let recommendation = stats.recommendation(threshold);

    Ok(RoutedInputStressReport {
        display_name: display_name.map(str::to_owned),
        extension_opcode: harness.routed_major_opcode,
        target_window: harness.target_window(),
        device: harness.device,
        iterations,
        accepted,
        request_bytes,
        threshold,
        stats,
        recommendation,
    })
}
