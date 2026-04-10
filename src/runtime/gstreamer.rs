use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_rtsp_server as gst_rtsp_server;
use gstreamer_rtsp_server::prelude::*;

use super::events::{emit_runtime_failure_events, stream_event_from_message};
use super::probe::select_pipeline;
use crate::{StartupDiagnostic, StreamConfig, StreamError, StreamEvent};

type RtspThreadResult = std::thread::Result<Result<Result<(), StreamError>, gst::glib::BoolError>>;

#[derive(Clone, Default)]
pub(crate) struct SessionHandle {
    inner: Option<Arc<SessionInner>>,
}

struct SessionStartup {
    pipeline_label: String,
    startup_diagnostics: Vec<StartupDiagnostic>,
    pipeline_description: String,
    stream_url: String,
    bind_host: String,
    bind_port: String,
    stream_path: String,
}

struct RtspThreadConfig {
    bind_host: String,
    bind_port: String,
    stream_path: String,
    launch_description: String,
    stream_url: String,
}

struct SessionInner {
    main_loop: gst::glib::MainLoop,
    thread_handle: Mutex<Option<thread::JoinHandle<()>>>,
    event_rx: Mutex<mpsc::Receiver<StreamEvent>>,
    #[allow(dead_code)]
    pipeline_label: String,
    #[allow(dead_code)]
    startup_diagnostics: Vec<StartupDiagnostic>,
    #[allow(dead_code)]
    pipeline_description: String,
    #[allow(dead_code)]
    stream_url: String,
}

impl std::fmt::Debug for SessionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionHandle").finish_non_exhaustive()
    }
}

impl SessionHandle {
    pub(crate) fn inert() -> Self {
        Self::default()
    }

    pub(crate) fn start(config: &StreamConfig) -> Result<Self, StreamError> {
        gst::init().map_err(|error| StreamError::InitFailed(error.to_string()))?;

        let startup = SessionStartup::from_config(config)?;
        let (started_tx, started_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let context = gst::glib::MainContext::new();
        let main_loop = gst::glib::MainLoop::new(Some(&context), false);
        let thread_handle = spawn_rtsp_thread(
            &context,
            &main_loop,
            startup.thread_config(),
            started_tx,
            event_tx,
        );

        await_startup(&started_rx)?;

        Ok(Self {
            inner: Some(Arc::new(SessionInner {
                main_loop,
                thread_handle: Mutex::new(Some(thread_handle)),
                event_rx: Mutex::new(event_rx),
                pipeline_label: startup.pipeline_label,
                startup_diagnostics: startup.startup_diagnostics,
                pipeline_description: startup.pipeline_description,
                stream_url: startup.stream_url,
            })),
        })
    }

    pub(crate) fn stop(&self) -> Result<(), StreamError> {
        if let Some(inner) = &self.inner {
            shutdown_inner(inner)?;
        }

        Ok(())
    }

    pub(crate) fn poll_event(&self, timeout: Duration) -> Option<StreamEvent> {
        let inner = self.inner.as_ref()?;
        let event_rx = inner.event_rx.lock().ok()?;
        event_rx.recv_timeout(timeout).ok()
    }

    pub(crate) fn pipeline_label(&self) -> String {
        self.inner
            .as_ref()
            .map(|inner| inner.pipeline_label.clone())
            .unwrap_or_default()
    }

    pub(crate) fn startup_diagnostics(&self) -> Vec<StartupDiagnostic> {
        self.inner
            .as_ref()
            .map(|inner| inner.startup_diagnostics.clone())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn pipeline_description(&self) -> String {
        self.inner
            .as_ref()
            .map(|inner| inner.pipeline_description.clone())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn stream_url(&self) -> String {
        self.inner
            .as_ref()
            .map(|inner| inner.stream_url.clone())
            .unwrap_or_default()
    }
}

impl SessionStartup {
    fn from_config(config: &StreamConfig) -> Result<Self, StreamError> {
        let selected_pipeline = select_pipeline(config)?;

        Ok(Self {
            pipeline_label: selected_pipeline.label.to_string(),
            startup_diagnostics: selected_pipeline.startup_diagnostics,
            pipeline_description: selected_pipeline.description,
            stream_url: format!(
                "rtsp://{}:{}{}",
                config.bind_host(),
                config.listen_port(),
                config.stream_path()
            ),
            bind_host: config.bind_host().to_string(),
            bind_port: config.listen_port().to_string(),
            stream_path: config.stream_path().to_string(),
        })
    }

    fn thread_config(&self) -> RtspThreadConfig {
        RtspThreadConfig {
            bind_host: self.bind_host.clone(),
            bind_port: self.bind_port.clone(),
            stream_path: self.stream_path.clone(),
            launch_description: self.pipeline_description.clone(),
            stream_url: self.stream_url.clone(),
        }
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        if let Some(inner) = &self.inner {
            let _ = shutdown_inner(inner);
        }
    }
}

fn shutdown_inner(inner: &SessionInner) -> Result<(), StreamError> {
    inner.main_loop.quit();

    if let Some(thread_handle) = inner
        .thread_handle
        .lock()
        .map_err(|_| StreamError::RuntimeError("failed to lock RTSP thread handle".to_string()))?
        .take()
    {
        let _ = thread_handle.join();
    }

    Ok(())
}

fn spawn_rtsp_thread(
    context: &gst::glib::MainContext,
    main_loop: &gst::glib::MainLoop,
    thread_config: RtspThreadConfig,
    started_tx: mpsc::Sender<Result<(), StreamError>>,
    event_tx: mpsc::Sender<StreamEvent>,
) -> thread::JoinHandle<()> {
    let context = context.clone();
    let main_loop = main_loop.clone();

    thread::spawn(move || {
        let (startup_reported, result) =
            run_rtsp_thread(context, main_loop, thread_config, &started_tx, &event_tx);
        handle_rtsp_thread_result(result, startup_reported, &started_tx, &event_tx);
    })
}

fn run_rtsp_thread(
    context: gst::glib::MainContext,
    main_loop: gst::glib::MainLoop,
    thread_config: RtspThreadConfig,
    started_tx: &mpsc::Sender<Result<(), StreamError>>,
    event_tx: &mpsc::Sender<StreamEvent>,
) -> (bool, RtspThreadResult) {
    let mut startup_reported = false;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        context.with_thread_default(|| {
            launch_rtsp_server(
                &context,
                &main_loop,
                &thread_config,
                started_tx,
                event_tx,
                &mut startup_reported,
            )
        })
    }));

    (startup_reported, result)
}

fn launch_rtsp_server(
    context: &gst::glib::MainContext,
    main_loop: &gst::glib::MainLoop,
    thread_config: &RtspThreadConfig,
    started_tx: &mpsc::Sender<Result<(), StreamError>>,
    event_tx: &mpsc::Sender<StreamEvent>,
    startup_reported: &mut bool,
) -> Result<(), StreamError> {
    let server = gst_rtsp_server::RTSPServer::new();
    server.set_address(&thread_config.bind_host);
    server.set_service(&thread_config.bind_port);

    let mounts = server.mount_points().ok_or_else(|| {
        StreamError::RuntimeError("failed to retrieve RTSP mount points".to_string())
    })?;
    let factory = build_media_factory(&thread_config.launch_description, event_tx);
    mounts.add_factory(&thread_config.stream_path, factory);

    server
        .attach(Some(context))
        .map_err(|_| StreamError::RuntimeError("failed to attach RTSP server".to_string()))?;

    *startup_reported = true;
    let started_tx = started_tx.clone();
    let started_event_tx = event_tx.clone();
    let stream_url = thread_config.stream_url.clone();
    context.invoke(move || {
        let _ = started_tx.send(Ok(()));
        let _ = started_event_tx.send(StreamEvent::Started { stream_url });
    });

    main_loop.run();

    let _ = event_tx.send(StreamEvent::Stopped { reason: None });

    Ok(())
}

fn build_media_factory(
    launch_description: &str,
    event_tx: &mpsc::Sender<StreamEvent>,
) -> gst_rtsp_server::RTSPMediaFactory {
    let factory = gst_rtsp_server::RTSPMediaFactory::new();
    factory.set_shared(true);
    factory.set_launch(&format!("( {launch_description} )"));

    let event_tx_for_media = event_tx.clone();
    factory.connect_media_configure(move |_, media| {
        let element = media.element();

        let Ok(pipeline) = element.dynamic_cast::<gst::Pipeline>() else {
            return;
        };

        let Some(bus): Option<gst::Bus> = pipeline.bus() else {
            return;
        };

        let event_tx_for_bus = event_tx_for_media.clone();
        let _ = bus.add_watch_local(move |_, message| {
            if let Some(event) = stream_event_from_message(message) {
                let _ = event_tx_for_bus.send(event);
            }

            gst::glib::ControlFlow::Continue
        });
    });

    factory
}

fn handle_rtsp_thread_result(
    result: RtspThreadResult,
    startup_reported: bool,
    started_tx: &mpsc::Sender<Result<(), StreamError>>,
    event_tx: &mpsc::Sender<StreamEvent>,
) {
    match result {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(error))) => report_rtsp_thread_failure(
            stream_error_message(error),
            startup_reported,
            started_tx,
            event_tx,
        ),
        Ok(Err(error)) => {
            report_rtsp_thread_failure(error.to_string(), startup_reported, started_tx, event_tx)
        }
        Err(payload) => report_rtsp_thread_failure(
            panic_message(payload),
            startup_reported,
            started_tx,
            event_tx,
        ),
    }
}

fn report_rtsp_thread_failure(
    message: String,
    startup_reported: bool,
    started_tx: &mpsc::Sender<Result<(), StreamError>>,
    event_tx: &mpsc::Sender<StreamEvent>,
) {
    if startup_reported {
        emit_runtime_failure_events(event_tx, message);
    } else {
        let _ = started_tx.send(Err(StreamError::RuntimeError(message)));
    }
}

fn await_startup(started_rx: &mpsc::Receiver<Result<(), StreamError>>) -> Result<(), StreamError> {
    started_rx.recv().map_err(|_| {
        StreamError::RuntimeError("failed to receive RTSP startup status".to_string())
    })?
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic in RTSP thread".to_string()
    }
}

fn stream_error_message(error: StreamError) -> String {
    match error {
        StreamError::InvalidConfig(message)
        | StreamError::InitFailed(message)
        | StreamError::PipelineBuildFailed(message)
        | StreamError::StateChangeFailed(message)
        | StreamError::RuntimeError(message) => message,
    }
}
