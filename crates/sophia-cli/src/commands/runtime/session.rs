use super::*;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if args.iter().any(|arg| arg == "runtime-damage-epoch-smoke") {
        let engine = HeadlessEngine::default();
        let output = engine.output();
        let surface = SurfaceId::new(1, 1);
        let frame_serial = 5;
        let mut epoch = LayoutEpochState::with_timing(1, [surface], 0, 300);
        let damage = DamageFrame {
            output: output.id,
            frame_serial,
            buffer_age: 1,
            root_generation: 1,
            affected_surfaces: vec![surface],
            damage: Region::single(Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }),
        };
        let tick = FrameClockTick {
            output: output.id,
            frame_serial,
            target_msec: 80,
        };
        let decision = schedule_frame_from_damage(tick, Some(damage), Some(&mut epoch));
        let (scheduled_frame_serial, completed_epoch) = match decision {
            FrameScheduleDecision::Render {
                frame_serial,
                completed_epoch,
                ..
            } => (frame_serial, completed_epoch),
            other => {
                return Err(std::io::Error::other(format!(
                    "expected render decision, got {other:?}"
                ))
                .into());
            }
        };

        let mut driver = HeadlessSessionDriver::new(engine);
        let report = driver.run_tick(HeadlessSessionDriverTick {
            output: output.id,
            frame_serial: scheduled_frame_serial,
            x_event_count: 1,
            layers: synthetic_layers(),
            wm_update: None,
            portal_commands: Vec::new(),
            chrome_command_count: 0,
        })?;

        println!(
            "runtime-damage-epoch-smoke output={} frame_serial={} completed_epoch={:?} pending_surfaces={} runtime_phase={:?} runtime_commands={} runtime_frames={} runtime_x_events={}",
            output.id.raw(),
            scheduled_frame_serial,
            completed_epoch,
            epoch.pending_surfaces().len(),
            report.runtime_state.phase,
            report.runtime_commands.len(),
            report.runtime_state.frames_rendered,
            report.runtime_state.x_events_polled
        );
        return Ok(true);
    }

    if args
        .iter()
        .any(|arg| arg == "headless-session-driver-smoke")
    {
        let engine = HeadlessEngine::default();
        let output = engine.output();
        let mut driver = HeadlessSessionDriver::new(engine);
        let transaction = TransactionId::from_raw(30);
        let report = driver.run_tick(HeadlessSessionDriverTick {
            output: output.id,
            frame_serial: 6,
            x_event_count: 1,
            layers: synthetic_layers(),
            wm_update: Some(WmTransactionUpdate {
                commit: TransactionCommit {
                    transaction,
                    outcome: TransactionOutcome::Committed,
                    applied_surfaces: vec![SurfaceId::new(1, 1)],
                },
                ipc_error: None,
            }),
            portal_commands: vec![PortalCommand::DropNotification {
                transfer: PortalTransferId::from_raw(1),
            }],
            chrome_command_count: 1,
        })?;
        let session_tick = report
            .session_tick
            .as_ref()
            .ok_or("headless session driver did not render a frame")?;

        println!(
            "headless-session-driver-smoke output={} frame_serial={} runtime_phase={:?} runtime_commands={} runtime_frames={} runtime_x_events={} runtime_portal={} runtime_chrome={} cached_layers={} frame_layers={} replay_steps={}",
            output.id.raw(),
            session_tick.frame.frame_serial,
            report.runtime_state.phase,
            report.runtime_commands.len(),
            report.runtime_state.frames_rendered,
            report.runtime_state.x_events_polled,
            report.runtime_state.portal_commands_drained,
            report.runtime_state.chrome_commands_presented,
            report.cached_layers,
            session_tick.frame.layers.len(),
            session_tick.replay.steps.len()
        );
        return Ok(true);
    }

    Ok(false)
}
