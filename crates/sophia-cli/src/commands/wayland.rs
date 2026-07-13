use super::prelude::*;

use sophia_cli::emergency_input::{EmergencyChordAction, EmergencyChordState};
use sophia_engine::{
    FocusedInputRoute, InputFocusState, NonBlockingInputPoller, hit_test_scene_surface_for_input,
    routed_input_request_from_physical_event,
};
use sophia_protocol::{
    AuthorityFeedback, BufferSource, CpuBufferFormat, CpuBufferRegistration, DeviceId,
    InputEventKind, Point, RoutedInputRequest, SeatId, SurfacePresentationFeedback,
};
use sophia_wayland_authority::{
    DmaBufRegistration, WaylandAuthorityAction, WaylandFrontend, WaylandFrontendEvent,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

const DEFAULT_OUTPUT_SIZE: Size = Size {
    width: 1280,
    height: 720,
};

pub(crate) fn try_run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    if !args.iter().any(|arg| arg == "sophia-wayland-session") {
        return Ok(false);
    }
    run_session(args)?;
    Ok(true)
}

pub(crate) fn run_session(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let client = arg_value(args, "--client").ok_or("--client=PATH is required")?;
    let client_args = args
        .iter()
        .filter_map(|arg| arg.strip_prefix("--client-arg=").map(str::to_owned))
        .collect::<Vec<_>>();
    if client_args.len() > 64 || client_args.iter().any(|arg| arg.len() > 4096) {
        return Err("--client-arg accepts at most 64 bounded arguments".into());
    }
    let display_name = arg_value(args, "--wayland-display")
        .unwrap_or_else(|| format!("sophia-{}", std::process::id()));
    let max_runtime = arg_value(args, "--max-runtime-ms")
        .map(|value| value.parse::<u64>())
        .transpose()?
        .map(Duration::from_millis);
    let resize = arg_value(args, "--resize")
        .map(|value| parse_size(&value))
        .transpose()?;
    let resize_after = arg_value(args, "--resize-after-ms")
        .map(|value| value.parse::<u64>())
        .transpose()?
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_millis(750));
    let input_devices = arg_value(args, "--input-devices")
        .map(|value| {
            value
                .split(',')
                .filter(|path| !path.is_empty())
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let expect_input_pixel_change = args.iter().any(|arg| arg == "--expect-input-pixel-change");
    let expect_input_presentation = args.iter().any(|arg| arg == "--expect-input-presentation");
    let expect_pointer_input = args.iter().any(|arg| arg == "--expect-pointer-input");
    let expected_keycodes = arg_value(args, "--expect-keycodes")
        .map(|value| parse_keycodes(&value))
        .transpose()?
        .unwrap_or_default();
    let max_input_latency = arg_value(args, "--max-input-latency-ms")
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(100);
    let mut native_scanout = args
        .iter()
        .any(|arg| arg == "--native-scanout")
        .then(super::live_session::WaylandNativeSession::new)
        .transpose()?;
    let output_size = native_scanout
        .as_ref()
        .map(super::live_session::WaylandNativeSession::primary_size)
        .unwrap_or(DEFAULT_OUTPUT_SIZE);
    if resize.is_some_and(|size| {
        size.width <= 0
            || size.height <= 0
            || size.width > output_size.width
            || size.height > output_size.height
            || size == output_size
    }) {
        return Err("--resize must be a positive size smaller than the output".into());
    }
    let mut frontend = if native_scanout.is_some() {
        WaylandFrontend::bind_with_dmabuf(
            &display_name,
            output_size,
            native_scanout
                .as_ref()
                .expect("checked above")
                .dmabuf_main_device()?,
        )?
    } else {
        WaylandFrontend::bind(&display_name, output_size)?
    };
    let mut child = spawn_client(&client, &client_args, &display_name)?;
    let engine = HeadlessEngine::new(sophia_engine::HeadlessOutput {
        id: sophia_protocol::OutputId::from_raw(1),
        size: output_size,
        scale: 1,
    });
    let mut committed: Vec<CommittedSurfaceState> = Vec::new();
    let mut focus = InputFocusState::new();
    let mut scene = WaylandCpuScene::new(output_size);
    let mut input = if input_devices.is_empty() {
        None
    } else {
        Some(sophia_backend_live::open_native_libinput_path_poller(
            &input_devices,
            sophia_backend_live::NativeLibinputDeviceMap::new(SeatId::from_raw(1))
                .with_keyboard_device(DeviceId::from_raw(1))
                .with_pointer_device(DeviceId::from_raw(2)),
            64,
        )?)
    };
    let mut emergency_chord = EmergencyChordState::armed();
    let mut pending_pixel_input = VecDeque::new();
    let mut pending_presented_input = VecDeque::new();
    let mut pending_presented_pointer = VecDeque::new();
    let mut presentation_observations = BTreeMap::new();
    let mut routed_input = 0usize;
    let mut routed_keys = 0usize;
    let mut routed_pointer = 0usize;
    let mut observed_keycodes = BTreeSet::new();
    let mut presented_keycodes = BTreeSet::new();
    let mut input_pixel_changes = 0usize;
    let mut input_presentations = 0usize;
    let mut pointer_presentations = 0usize;
    let mut max_observed_input_latency = Duration::ZERO;
    let mut last_checksum = None;
    let started = Instant::now();
    let mut transactions = 0usize;
    let mut frames = 0usize;
    let mut shm_frames = 0usize;
    let mut dmabuf_frames = 0usize;
    let mut resize_requested = false;
    let mut resize_commits = 0usize;

    println!(
        "sophia_wayland_session schema=1 status=running display={} client={} x_server=disabled",
        display_name, client
    );

    loop {
        let retired_presentations = native_scanout
            .as_mut()
            .map(super::live_session::WaylandNativeSession::service)
            .transpose()?
            .unwrap_or_default();
        for (surface, generation) in retired_presentations {
            let checksum = presentation_observations
                .remove(&(surface, generation))
                .flatten();
            observe_input_presentation(
                checksum,
                &mut pending_pixel_input,
                &mut pending_presented_input,
                &mut pending_presented_pointer,
                &mut presented_keycodes,
                &mut input_pixel_changes,
                &mut input_presentations,
                &mut pointer_presentations,
                &mut max_observed_input_latency,
            );
            let item = AuthorityFeedback::Presented(SurfacePresentationFeedback {
                surface,
                generation,
                presentation_msec: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            });
            for event in frontend.apply_feedback(item)? {
                if let WaylandFrontendEvent::Authority(WaylandAuthorityAction::BufferReleased(
                    release,
                )) = event
                {
                    scene.release(release.source);
                }
            }
        }
        if max_runtime.is_some_and(|limit| started.elapsed() >= limit) {
            break;
        }
        if let Some(status) = child.try_wait()? {
            if status.success() {
                break;
            }
            return Err(format!("Wayland client exited with status {status}").into());
        }
        if !resize_requested
            && resize.is_some()
            && started.elapsed() >= resize_after
            && let Some(surface) = committed.first().map(|state| state.surface)
        {
            let size = resize.expect("checked above");
            if frontend.configure_toplevel(surface, size)? {
                resize_requested = true;
                println!(
                    "sophia_wayland_resize schema=1 status=requested surface={} width={} height={}",
                    surface.index(),
                    size.width,
                    size.height,
                );
            }
        }

        if let Some(input) = input.as_mut() {
            for event in input.poll_ready()? {
                if let InputEventKind::Key { keycode, pressed } = event.kind
                    && emergency_chord.observe(keycode, pressed) == EmergencyChordAction::Triggered
                {
                    terminate_client(&mut child)?;
                    println!("sophia_wayland_input schema=1 status=emergency_exit");
                    return Ok(());
                }
                let request = match event.kind {
                    InputEventKind::Key { .. } => {
                        let FocusedInputRoute::Routed(event) =
                            focus.route_keyboard_event(event, &committed)
                        else {
                            continue;
                        };
                        let Some(target_surface) = event.target_surface else {
                            continue;
                        };
                        RoutedInputRequest {
                            serial: event.serial,
                            seat: event.seat,
                            device: event.device,
                            time_msec: event.time_msec,
                            target_surface,
                            global_position: Point::default(),
                            local_position: Point::default(),
                            kind: event.kind,
                        }
                    }
                    InputEventKind::PointerMotion | InputEventKind::PointerButton { .. } => {
                        let route =
                            hit_test_scene_surface_for_input(&event, &input_layers(&committed));
                        if matches!(
                            event.kind,
                            InputEventKind::PointerButton { pressed: true, .. }
                        ) && let Some(surface) = route.target_surface
                        {
                            let _ = focus.focus_surface(event.seat, surface, &committed);
                        }
                        let Ok(request) = routed_input_request_from_physical_event(&event, &route)
                        else {
                            continue;
                        };
                        request
                    }
                };
                let decision = frontend.route_input(&request);
                if decision.outcome == sophia_protocol::RoutedInputOutcome::Accepted {
                    routed_input = routed_input.saturating_add(1);
                    match request.kind {
                        InputEventKind::Key { keycode, pressed } => {
                            routed_keys = routed_keys.saturating_add(1);
                            if pressed {
                                observed_keycodes.insert(keycode);
                                let observed = Instant::now();
                                pending_pixel_input.push_back((observed, last_checksum));
                                pending_presented_input.push_back((observed, keycode));
                            }
                        }
                        InputEventKind::PointerMotion | InputEventKind::PointerButton { .. } => {
                            routed_pointer = routed_pointer.saturating_add(1);
                            pending_presented_pointer.push_back(Instant::now());
                        }
                    }
                }
            }
        }

        let events = frontend.dispatch()?;
        let mut feedback = Vec::new();
        for event in events {
            match event {
                WaylandFrontendEvent::CpuBufferRegistered(buffer) => scene.register(buffer),
                WaylandFrontendEvent::DmaBufRegistered(buffer) => scene.register_dmabuf(buffer),
                WaylandFrontendEvent::Authority(WaylandAuthorityAction::BufferReleased(
                    release,
                )) => scene.release(release.source),
                WaylandFrontendEvent::Authority(WaylandAuthorityAction::SurfaceTransaction(
                    transaction,
                )) => {
                    let transaction_id = transaction.transaction;
                    let surface = transaction.surface;
                    let commit = engine.commit_surface_transactions(
                        transaction_id,
                        std::slice::from_ref(&transaction),
                        &mut committed,
                    );
                    transactions = transactions.saturating_add(1);
                    let committed_generation = (commit.outcome == TransactionOutcome::Committed
                        && commit.applied_surfaces.contains(&surface))
                    .then(|| {
                        committed
                            .iter()
                            .find(|state| state.surface == surface)
                            .map(|state| state.committed_generation)
                    })
                    .flatten();
                    if committed_generation.is_some()
                        && resize_requested
                        && resize.is_some_and(|size| {
                            transaction.target_geometry.width == size.width
                                && transaction.target_geometry.height == size.height
                        })
                    {
                        resize_commits = resize_commits.saturating_add(1);
                    }
                    if focus.focused_surface(SeatId::from_raw(1)).is_none() {
                        let _ = focus.focus_surface(SeatId::from_raw(1), surface, &committed);
                    }
                    scene.observe_committed(&committed);
                    feedback.push(AuthorityFeedback::Transaction(commit));
                    if let Some(generation) = committed_generation {
                        let report =
                            if matches!(transaction.target_buffer, BufferSource::DmaBuf { .. }) {
                                None
                            } else {
                                Some(scene.compose()?)
                            };
                        let presented = match transaction.target_buffer {
                            BufferSource::DmaBuf { handle } => {
                                dmabuf_frames = dmabuf_frames.saturating_add(1);
                                let frame = scene.dmabuf_frame(handle)?;
                                let native_scanout = native_scanout
                                    .as_mut()
                                    .ok_or("DMA-BUF client buffers require --native-scanout")?;
                                native_scanout.present_dmabuf(&transaction, generation, &frame)?
                            }
                            _ => {
                                shm_frames = shm_frames.saturating_add(1);
                                let report = report.as_ref().expect("CPU report created above");
                                if let Some(native_scanout) = native_scanout.as_mut() {
                                    native_scanout.present(&transaction, generation, &report)?
                                } else {
                                    true
                                }
                            }
                        };
                        frames = frames.saturating_add(1);
                        let checksum = report.as_ref().map(|report| report.checksum);
                        if let Some(checksum) = checksum {
                            last_checksum = Some(checksum);
                        }
                        let (source, checksum_value, nonzero_pixel_bytes) = match report.as_ref() {
                            Some(report) => ("shm", report.checksum, report.nonzero_pixel_bytes),
                            None => ("dmabuf", 0, 0),
                        };
                        println!(
                            "sophia_wayland_frame schema=1 surface={} generation={} width={} height={} buffer={} checksum={} nonzero_pixel_bytes={}",
                            surface.index(),
                            generation,
                            transaction.target_geometry.width,
                            transaction.target_geometry.height,
                            source,
                            checksum_value,
                            nonzero_pixel_bytes
                        );
                        if presented {
                            observe_input_presentation(
                                checksum,
                                &mut pending_pixel_input,
                                &mut pending_presented_input,
                                &mut pending_presented_pointer,
                                &mut presented_keycodes,
                                &mut input_pixel_changes,
                                &mut input_presentations,
                                &mut pointer_presentations,
                                &mut max_observed_input_latency,
                            );
                            feedback.push(AuthorityFeedback::Presented(
                                SurfacePresentationFeedback {
                                    surface,
                                    generation,
                                    presentation_msec: u64::try_from(started.elapsed().as_millis())
                                        .unwrap_or(u64::MAX),
                                },
                            ));
                        } else {
                            presentation_observations.insert((surface, generation), checksum);
                        }
                    }
                }
                WaylandFrontendEvent::Authority(WaylandAuthorityAction::SurfaceDestroyed {
                    surface,
                }) => {
                    committed.retain(|state| state.surface != surface);
                    presentation_observations.retain(|(candidate, _), _| *candidate != surface);
                    focus.clear_surface(surface);
                    if let Some(next) = committed.first() {
                        let _ = focus.focus_surface(SeatId::from_raw(1), next.surface, &committed);
                    }
                    scene.observe_committed(&committed);
                    if let Some(native_scanout) = native_scanout.as_mut() {
                        native_scanout.cancel_surface(surface);
                    }
                }
                WaylandFrontendEvent::ProtocolError(error) => {
                    return Err(format!("Wayland protocol authority failed: {error}").into());
                }
                _ => {}
            }
        }
        for item in feedback {
            for event in frontend.apply_feedback(item)? {
                if let WaylandFrontendEvent::Authority(WaylandAuthorityAction::BufferReleased(
                    release,
                )) = event
                {
                    scene.release(release.source);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    terminate_client(&mut child)?;
    if let Some(native_scanout) = native_scanout.as_mut() {
        native_scanout.shutdown()?;
        println!("{}", native_scanout.completion_evidence());
    }
    if expect_input_pixel_change && (routed_input == 0 || input_pixel_changes == 0) {
        return Err("Wayland input proof did not produce a presented pixel change".into());
    }
    if expect_input_presentation && (routed_input == 0 || input_presentations == 0) {
        return Err("Wayland input proof did not reach a presented client frame".into());
    }
    let observed_required_keycodes = expected_keycodes.intersection(&observed_keycodes).count();
    let matched_keycodes = expected_keycodes.intersection(&presented_keycodes).count();
    if matched_keycodes != expected_keycodes.len() {
        return Err(format!(
            "Wayland input proof matched {matched_keycodes}/{} required keycodes",
            expected_keycodes.len()
        )
        .into());
    }
    if expect_pointer_input && routed_pointer == 0 {
        return Err("Wayland input proof observed no routed pointer input".into());
    }
    if expect_pointer_input && pointer_presentations == 0 {
        return Err("Wayland pointer input did not reach a presented client frame".into());
    }
    if (expect_input_pixel_change || expect_input_presentation)
        && max_observed_input_latency > Duration::from_millis(max_input_latency)
    {
        return Err(format!(
            "Wayland input presentation latency {}ms exceeds {}ms",
            max_observed_input_latency.as_millis(),
            max_input_latency
        )
        .into());
    }
    println!(
        "sophia_wayland_session schema=1 status=complete transactions={} frames={} shm_frames={} dmabuf_frames={} resize_requested={} resize_commits={} buffers={} routed_input={} routed_keys={} routed_pointer={} expected_keycodes_observed={} expected_keycodes_matched={} expected_keycodes_total={} input_presentations={} pointer_presentations={} input_pixel_changes={} max_input_latency_msec={} x_server=disabled",
        transactions,
        frames,
        shm_frames,
        dmabuf_frames,
        usize::from(resize_requested),
        resize_commits,
        scene.buffer_count(),
        routed_input,
        routed_keys,
        routed_pointer,
        observed_required_keycodes,
        matched_keycodes,
        expected_keycodes.len(),
        input_presentations,
        pointer_presentations,
        input_pixel_changes,
        max_observed_input_latency.as_millis(),
    );
    Ok(())
}

fn parse_size(value: &str) -> Result<Size, Box<dyn std::error::Error>> {
    let (width, height) = value.split_once('x').ok_or("size must use WIDTHxHEIGHT")?;
    Ok(Size {
        width: width.parse()?,
        height: height.parse()?,
    })
}

fn parse_keycodes(value: &str) -> Result<BTreeSet<u32>, Box<dyn std::error::Error>> {
    let keycodes = value
        .split(',')
        .filter(|item| !item.is_empty())
        .map(str::parse)
        .collect::<Result<BTreeSet<u32>, _>>()?;
    if keycodes.is_empty() || keycodes.len() > 32 || keycodes.contains(&0) {
        return Err("--expect-keycodes requires 1..32 nonzero evdev keycodes".into());
    }
    Ok(keycodes)
}

#[allow(clippy::too_many_arguments)]
fn observe_input_presentation(
    checksum: Option<u64>,
    pending_pixel_input: &mut VecDeque<(Instant, Option<u64>)>,
    pending_presented_input: &mut VecDeque<(Instant, u32)>,
    pending_presented_pointer: &mut VecDeque<Instant>,
    presented_keycodes: &mut BTreeSet<u32>,
    input_pixel_changes: &mut usize,
    input_presentations: &mut usize,
    pointer_presentations: &mut usize,
    max_observed_input_latency: &mut Duration,
) {
    while let Some((started, keycode)) = pending_presented_input.pop_front() {
        *max_observed_input_latency = (*max_observed_input_latency).max(started.elapsed());
        *input_presentations = input_presentations.saturating_add(1);
        presented_keycodes.insert(keycode);
    }
    while let Some(started) = pending_presented_pointer.pop_front() {
        *max_observed_input_latency = (*max_observed_input_latency).max(started.elapsed());
        *pointer_presentations = pointer_presentations.saturating_add(1);
    }
    let Some(checksum) = checksum else {
        return;
    };
    if !pending_pixel_input
        .front()
        .is_some_and(|(_, baseline)| baseline.is_none_or(|baseline| baseline != checksum))
    {
        return;
    }
    while let Some((started, _)) = pending_pixel_input.pop_front() {
        *max_observed_input_latency = (*max_observed_input_latency).max(started.elapsed());
        *input_pixel_changes = input_pixel_changes.saturating_add(1);
    }
}

fn input_layers(committed: &[CommittedSurfaceState]) -> Vec<LayerSnapshot> {
    committed
        .iter()
        .enumerate()
        .map(|(stack_rank, surface)| LayerSnapshot {
            surface: surface.surface,
            authority_local_id: None,
            namespace: None,
            stack_rank: u32::try_from(stack_rank).unwrap_or(u32::MAX),
            geometry: surface.geometry,
            source: surface.buffer,
            damage: surface.damage.clone(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: surface.committed_generation,
            resize_sync: ResizeSyncCapability::ImplicitOnly,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_keycodes, parse_size};

    #[test]
    fn parses_bounded_resize_and_keycode_proof_arguments() {
        let size = parse_size("1024x640").unwrap();
        assert_eq!(size.width, 1024);
        assert_eq!(size.height, 640);
        let keycodes = parse_keycodes("31,24,31,103").unwrap();
        assert_eq!(keycodes.into_iter().collect::<Vec<_>>(), vec![24, 31, 103]);
        assert!(parse_size("1024").is_err());
        assert!(parse_keycodes("0").is_err());
        assert!(parse_keycodes("").is_err());
    }
}

fn spawn_client(
    client: &str,
    args: &[String],
    display_name: &str,
) -> Result<Child, Box<dyn std::error::Error>> {
    Ok(std::process::Command::new(client)
        .args(args)
        .env("WAYLAND_DISPLAY", display_name)
        .env_remove("DISPLAY")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?)
}

fn terminate_client(child: &mut Child) -> Result<(), Box<dyn std::error::Error>> {
    if child.try_wait()?.is_none() {
        child.kill()?;
        let _ = child.wait()?;
    }
    Ok(())
}

struct WaylandCpuScene {
    output_size: Size,
    buffers: BTreeMap<u64, CpuBufferRegistration>,
    dmabufs: BTreeMap<u64, DmaBufRegistration>,
    committed: Vec<CommittedSurfaceState>,
}

impl WaylandCpuScene {
    fn new(output_size: Size) -> Self {
        Self {
            output_size,
            buffers: BTreeMap::new(),
            dmabufs: BTreeMap::new(),
            committed: Vec::new(),
        }
    }

    fn register(&mut self, buffer: CpuBufferRegistration) {
        let stale = self
            .buffers
            .get(&buffer.handle)
            .is_some_and(|current| current.generation > buffer.generation);
        if !stale {
            self.buffers.insert(buffer.handle, buffer);
        }
    }

    fn observe_committed(&mut self, committed: &[CommittedSurfaceState]) {
        self.committed = committed.to_vec();
    }

    fn register_dmabuf(&mut self, buffer: DmaBufRegistration) {
        self.dmabufs.insert(buffer.handle, buffer);
    }

    fn release(&mut self, source: BufferSource) {
        match source {
            BufferSource::CpuBuffer { handle } => {
                self.buffers.remove(&handle);
            }
            // DMA-BUF admission belongs to the wl_buffer resource, which may
            // be reattached after release. Keep its immutable plane metadata
            // for the lifetime of this bounded session.
            BufferSource::DmaBuf { .. } => {}
            _ => {}
        }
    }

    fn dmabuf_frame(
        &self,
        handle: u64,
    ) -> Result<sophia_backend_live::LiveOwnedDmaBufFrame, Box<dyn std::error::Error>> {
        let buffer = self
            .dmabufs
            .get(&handle)
            .ok_or("missing admitted DMA-BUF handle")?;
        let fd = buffer
            .dmabuf
            .handles()
            .next()
            .ok_or("DMA-BUF has no plane")?
            .try_clone_to_owned()?;
        Ok(sophia_backend_live::LiveOwnedDmaBufFrame {
            width: u32::try_from(buffer.size.width)?,
            height: u32::try_from(buffer.size.height)?,
            format: buffer.format,
            modifier: buffer.modifier,
            fd,
            offset: buffer.dmabuf.offsets().next().unwrap_or(0),
            stride: buffer.dmabuf.strides().next().unwrap_or(0),
        })
    }

    fn compose(
        &self,
    ) -> Result<sophia_backend_live::LiveCpuCompositionReport, Box<dyn std::error::Error>> {
        let layers = self
            .committed
            .iter()
            .filter_map(|surface| {
                let BufferSource::CpuBuffer { handle } = surface.buffer else {
                    return None;
                };
                let buffer = self.buffers.get(&handle)?;
                Some(sophia_backend_live::LiveCpuCompositionLayerRef {
                    geometry: surface.geometry,
                    buffer: sophia_backend_live::LiveCpuBufferSourceRef {
                        handle,
                        size: buffer.size,
                        stride: buffer.stride,
                        format: match buffer.format {
                            CpuBufferFormat::Argb8888 | CpuBufferFormat::Xrgb8888 => {
                                sophia_backend_live::LIVE_RENDERER_SCANOUT_FORMAT_XRGB8888
                            }
                        },
                        generation: buffer.generation,
                        bytes: &buffer.bytes,
                    },
                })
            })
            .collect::<Vec<_>>();
        sophia_backend_live::compose_live_cpu_frame_ref(self.output_size, &layers)
            .map_err(|error| format!("Wayland CPU composition failed: {error:?}").into())
    }

    fn buffer_count(&self) -> usize {
        self.buffers.len()
    }
}
