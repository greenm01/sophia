use super::*;

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if args.iter().any(|arg| arg == "runtime-brokers-smoke") {
        let portal = arg_value(args, "--portal").unwrap_or_else(|| "/usr/bin/true".to_owned());
        let metadata = arg_value(args, "--metadata").unwrap_or_else(|| "/usr/bin/true".to_owned());
        let mut supervisors = RuntimeBrokerSupervisors::new(
            ProcessLaunchSpec::new(&portal),
            ProcessLaunchSpec::new(&metadata),
        );
        let report = supervisors.start_placeholders()?;
        let mut portal_exit = report.portal_poll;
        let mut metadata_exit = report.metadata_poll;

        for _ in 0..100 {
            if portal_exit == Some(SupervisorEvent::ProcessExited)
                && metadata_exit == Some(SupervisorEvent::ProcessExited)
            {
                break;
            }
            let (portal_event, metadata_event) = supervisors.poll_all()?;
            portal_exit = portal_exit.or(portal_event);
            metadata_exit = metadata_exit.or(metadata_event);
            std::thread::sleep(Duration::from_millis(1));
        }
        supervisors.terminate_all()?;

        println!(
            "runtime-brokers-smoke portal={} metadata={} portal_start={:?} metadata_start={:?} portal_exit={:?} metadata_exit={:?}",
            portal,
            metadata,
            report.portal_start,
            report.metadata_start,
            portal_exit,
            metadata_exit
        );
        return Ok(true);
    }

    if args.iter().any(|arg| arg == "portal-broker-health-smoke") {
        return run_broker_health_smoke(BrokerKind::Portal, args);
    }

    if args.iter().any(|arg| arg == "metadata-broker-health-smoke") {
        return run_broker_health_smoke(BrokerKind::Metadata, args);
    }

    Ok(false)
}

fn run_broker_health_smoke(
    broker: BrokerKind,
    _args: &[String],
) -> Result<bool, Box<dyn std::error::Error>> {
    let packet = BrokerHealthPacket::new(
        broker,
        BrokerHealthState::Ready,
        1,
        Some("placeholder ready".to_owned()),
    )
    .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let frame = encode_broker_health_frame(&packet)
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let decoded = decode_broker_health_frame(&frame)
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let message_len = decoded.message.as_deref().map(str::len).unwrap_or(0);
    let mut runtime = SessionRuntimeLoop::default();
    let runtime_report =
        runtime.step_observations([SessionRuntimeObservation::BrokerHealthChanged {
            broker: decoded.broker,
            state: decoded.state,
            generation: decoded.generation,
            status_message_len: message_len,
        }])?;
    let health = match decoded.broker {
        BrokerKind::Portal => runtime.state().portal_broker_health,
        BrokerKind::Metadata => runtime.state().metadata_broker_health,
    };
    let command = runtime_report
        .commands
        .first()
        .copied()
        .unwrap_or(SessionRuntimeCommand::None);

    println!(
        "{}-broker-health-smoke broker={:?} state={:?} generation={} message_len={} frame_bytes={} runtime_health={:?} runtime_command={:?}",
        broker_label(decoded.broker),
        decoded.broker,
        decoded.state,
        decoded.generation,
        message_len,
        frame.len(),
        health,
        command
    );
    Ok(true)
}

fn broker_label(broker: BrokerKind) -> &'static str {
    match broker {
        BrokerKind::Portal => "portal",
        BrokerKind::Metadata => "metadata",
    }
}
