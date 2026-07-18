use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec, poll};

use crate::prelude::*;

use super::{
    NativeLibinputDeviceMap, NativeLibinputOpenError, NativeLibinputPolicyReport,
    open_native_libinput_path_poller,
};

const INPUT_THREAD_POLL_MSEC: i64 = 1;

struct QueuedInputEvent {
    packet: InputEventPacket,
    queued_at: Instant,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ThreadedNativeInputStats {
    pub max_dispatch_gap_msec: usize,
    pub max_queue_depth: usize,
    pub max_queue_dwell_msec: usize,
}

pub struct ThreadedNativeLibinputEventPoller {
    receiver: Receiver<QueuedInputEvent>,
    health: Receiver<Result<(), String>>,
    policy: NativeLibinputPolicyReport,
    stop: Arc<AtomicBool>,
    queue_depth: Arc<AtomicUsize>,
    max_queue_depth: Arc<AtomicUsize>,
    max_dispatch_gap_msec: Arc<AtomicUsize>,
    max_queue_dwell_msec: usize,
    max_read_per_poll: usize,
    worker: Option<JoinHandle<()>>,
}

impl ThreadedNativeLibinputEventPoller {
    pub fn stats(&self) -> ThreadedNativeInputStats {
        ThreadedNativeInputStats {
            max_dispatch_gap_msec: self.max_dispatch_gap_msec.load(Ordering::Acquire),
            max_queue_depth: self.max_queue_depth.load(Ordering::Acquire),
            max_queue_dwell_msec: self.max_queue_dwell_msec,
        }
    }

    pub const fn policy_report(&self) -> NativeLibinputPolicyReport {
        self.policy
    }

    fn worker_error(&self) -> io::Result<()> {
        match self.health.try_recv() {
            Ok(Ok(())) | Err(TryRecvError::Empty) => Ok(()),
            Ok(Err(message)) => Err(io::Error::other(message)),
            Err(TryRecvError::Disconnected) if self.stop.load(Ordering::Acquire) => Ok(()),
            Err(TryRecvError::Disconnected) => Err(io::Error::other(
                "native input acquisition worker disconnected",
            )),
        }
    }
}

impl NonBlockingInputPoller for ThreadedNativeLibinputEventPoller {
    fn poll_ready(&mut self) -> io::Result<Vec<InputEventPacket>> {
        self.worker_error()?;
        let mut packets = Vec::new();
        while packets.len() < self.max_read_per_poll {
            let queued = match self.receiver.try_recv() {
                Ok(queued) => queued,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.worker_error()?;
                    break;
                }
            };
            self.queue_depth.fetch_sub(1, Ordering::AcqRel);
            self.max_queue_dwell_msec = self
                .max_queue_dwell_msec
                .max(usize::try_from(queued.queued_at.elapsed().as_millis()).unwrap_or(usize::MAX));
            packets.push(queued.packet);
        }
        self.worker_error()?;
        Ok(packets)
    }
}

impl Drop for ThreadedNativeLibinputEventPoller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn open_threaded_native_libinput_path_poller(
    paths: &[std::path::PathBuf],
    devices: NativeLibinputDeviceMap,
    max_read_per_poll: usize,
    queue_capacity: usize,
) -> Result<ThreadedNativeLibinputEventPoller, NativeLibinputOpenError> {
    if paths.is_empty() {
        return Err(NativeLibinputOpenError::NoDevices);
    }
    if paths.len() > 16 {
        return Err(NativeLibinputOpenError::TooManyDevices);
    }
    let paths = paths.to_vec();
    let max_read_per_poll = max_read_per_poll.clamp(1, 256);
    let queue_capacity = queue_capacity.clamp(1, 4_096);
    let (sender, receiver) = sync_channel(queue_capacity);
    let (startup_sender, startup_receiver) = sync_channel(1);
    let (health_sender, health) = sync_channel(1);
    let stop = Arc::new(AtomicBool::new(false));
    let queue_depth = Arc::new(AtomicUsize::new(0));
    let max_queue_depth = Arc::new(AtomicUsize::new(0));
    let max_dispatch_gap_msec = Arc::new(AtomicUsize::new(0));
    let worker_stop = Arc::clone(&stop);
    let worker_depth = Arc::clone(&queue_depth);
    let worker_max_depth = Arc::clone(&max_queue_depth);
    let worker_max_gap = Arc::clone(&max_dispatch_gap_msec);
    let worker = std::thread::spawn(move || {
        let mut poller = match open_native_libinput_path_poller(&paths, devices, max_read_per_poll)
        {
            Ok(poller) => {
                let policy = poller.reader().policy_report();
                let _ = startup_sender.send(Ok(policy));
                poller
            }
            Err(error) => {
                let _ = startup_sender.send(Err(error));
                return;
            }
        };
        let result = run_input_worker(
            &mut poller,
            sender,
            &worker_stop,
            &worker_depth,
            &worker_max_depth,
            &worker_max_gap,
        );
        let _ = health_sender.try_send(result);
    });
    match startup_receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(policy)) => Ok(ThreadedNativeLibinputEventPoller {
            receiver,
            health,
            policy,
            stop,
            queue_depth,
            max_queue_depth,
            max_dispatch_gap_msec,
            max_queue_dwell_msec: 0,
            max_read_per_poll,
            worker: Some(worker),
        }),
        Ok(Err(error)) => {
            let _ = worker.join();
            Err(error)
        }
        Err(_) => {
            stop.store(true, Ordering::Release);
            let _ = worker.join();
            Err(NativeLibinputOpenError::DeviceUnavailable)
        }
    }
}

fn run_input_worker(
    poller: &mut super::NativeLibinputEventPoller<super::NativeLibinputEventReader>,
    sender: SyncSender<QueuedInputEvent>,
    stop: &AtomicBool,
    queue_depth: &AtomicUsize,
    max_queue_depth: &AtomicUsize,
    max_dispatch_gap_msec: &AtomicUsize,
) -> Result<(), String> {
    let timeout = Timespec {
        tv_sec: 0,
        tv_nsec: INPUT_THREAD_POLL_MSEC * 1_000_000,
    };
    let mut last_dispatch = Instant::now();
    while !stop.load(Ordering::Acquire) {
        {
            let libinput = poller.reader().libinput_mut_ref();
            let mut fds = [PollFd::new(libinput, PollFlags::IN)];
            poll(&mut fds, Some(&timeout)).map_err(|error| error.to_string())?;
        }
        let gap = usize::try_from(last_dispatch.elapsed().as_millis()).unwrap_or(usize::MAX);
        observe_max(max_dispatch_gap_msec, gap);
        last_dispatch = Instant::now();
        let events = poller.poll_ready().map_err(|error| error.to_string())?;
        for packet in events {
            let depth = queue_depth.fetch_add(1, Ordering::AcqRel).saturating_add(1);
            observe_max(max_queue_depth, depth);
            match sender.try_send(QueuedInputEvent {
                packet,
                queued_at: Instant::now(),
            }) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    queue_depth.fetch_sub(1, Ordering::AcqRel);
                    return Err("native input acquisition queue saturated".to_owned());
                }
                Err(TrySendError::Disconnected(_)) => {
                    queue_depth.fetch_sub(1, Ordering::AcqRel);
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

fn observe_max(value: &AtomicUsize, candidate: usize) {
    let mut current = value.load(Ordering::Acquire);
    while candidate > current {
        match value.compare_exchange_weak(current, candidate, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::observe_max;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn concurrent_max_observation_never_regresses() {
        let value = AtomicUsize::new(3);
        observe_max(&value, 2);
        observe_max(&value, 9);
        observe_max(&value, 7);
        assert_eq!(value.load(Ordering::Acquire), 9);
    }
}
