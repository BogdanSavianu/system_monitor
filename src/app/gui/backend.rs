use std::{
    sync::mpsc,
    thread,
    time::{Duration, SystemTime},
};

use system_monitor::{dto::ProcessCpuSampleDTO, util::ParseError};
use tracing::info;

use crate::app::build_monitor;

#[derive(Debug, Clone)]
pub struct CpuSnapshot {
    pub collected_at: SystemTime,
    pub cpu: Vec<ProcessCpuSampleDTO>,
}

#[derive(Debug)]
pub enum BackendEvent {
    Snapshot(CpuSnapshot),
    Error(String),
    Stopped,
}

pub struct GuiBackendHandle {
    pub events_rx: mpsc::Receiver<BackendEvent>,
    shutdown_tx: mpsc::Sender<()>,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl GuiBackendHandle {
    pub fn shutdown(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

impl Drop for GuiBackendHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn spawn_backend(sample_interval: Duration) -> GuiBackendHandle {
    let (events_tx, events_rx) = mpsc::channel::<BackendEvent>();
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    let join_handle = thread::spawn(move || {
        let mut monitor = build_monitor();

        if let Err(err) = monitor.initialize_sampling() {
            let _ = events_tx.send(BackendEvent::Error(format!(
                "failed to initialize monitor: {:?}",
                err
            )));
            let _ = events_tx.send(BackendEvent::Stopped);
            return;
        }

        info!(target: "app::gui_backend", "gui backend initialized");

        loop {
            let snapshot = (|| -> Result<CpuSnapshot, ParseError> {
                let cpu = monitor.sample_cpu_usage()?;

                Ok(CpuSnapshot {
                    collected_at: SystemTime::now(),
                    cpu,
                })
            })();

            match snapshot {
                Ok(snapshot) => {
                    if events_tx.send(BackendEvent::Snapshot(snapshot)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ =
                        events_tx.send(BackendEvent::Error(format!("sampling failed: {:?}", err)));
                }
            }

            match shutdown_rx.recv_timeout(sample_interval) {
                Ok(()) => {
                    info!(target: "app::gui_backend", "gui backend shutdown requested");
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }

        let _ = events_tx.send(BackendEvent::Stopped);
    });

    GuiBackendHandle {
        events_rx,
        shutdown_tx,
        join_handle: Some(join_handle),
    }
}
