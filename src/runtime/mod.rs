use std::time::Duration;

use crate::{StartupDiagnostic, StreamConfig, StreamError, StreamEvent};

mod events;
mod gstreamer;
mod probe;
use gstreamer as backend;

#[derive(Debug, Clone)]
pub(crate) struct RuntimeSessionHandle {
    inner: backend::SessionHandle,
}

impl RuntimeSessionHandle {
    pub(crate) fn inert() -> Self {
        Self {
            inner: backend::SessionHandle::inert(),
        }
    }

    pub(crate) fn start(config: &StreamConfig) -> Result<Self, StreamError> {
        Ok(Self {
            inner: backend::SessionHandle::start(config)?,
        })
    }

    pub(crate) fn stop(&self) -> Result<(), StreamError> {
        self.inner.stop()
    }

    pub(crate) fn poll_event(&self, timeout: Duration) -> Option<StreamEvent> {
        self.inner.poll_event(timeout)
    }

    pub(crate) fn pipeline_label(&self) -> String {
        self.inner.pipeline_label()
    }

    pub(crate) fn startup_diagnostics(&self) -> Vec<StartupDiagnostic> {
        self.inner.startup_diagnostics()
    }

    #[cfg(test)]
    pub(crate) fn pipeline_description(&self) -> String {
        self.inner.pipeline_description()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{StreamConfig, StreamEvent, StreamSource, runtime::RuntimeSessionHandle};

    fn development_source() -> Option<StreamSource> {
        gstreamer::init().expect("gstreamer init should succeed");

        gstreamer::ElementFactory::find("videotestsrc").map(|_| StreamSource::videotest())
    }

    #[test]
    fn start_builds_pipeline_description() {
        let Some(source) = development_source() else {
            return;
        };

        let config = StreamConfig::new("127.0.0.1", 18554)
            .with_stream_path("/test")
            .with_source(source);

        let session = RuntimeSessionHandle::start(&config).expect("runtime start should succeed");

        assert_eq!(
            session.poll_event(Duration::from_millis(100)),
            Some(StreamEvent::Started {
                stream_url: "rtsp://127.0.0.1:18554/test".to_string(),
            })
        );
        assert!(session.pipeline_description().contains("videotestsrc"));
        assert!(session.pipeline_description().contains("vp8enc"));
        assert!(session.pipeline_description().contains("rtpvp8pay"));
        assert_eq!(session.pipeline_label(), "vp8");
        assert_eq!(session.inner.stream_url(), "rtsp://127.0.0.1:18554/test");
    }

    #[test]
    fn stop_emits_stopped_event() {
        let Some(source) = development_source() else {
            return;
        };

        let config = StreamConfig::new("127.0.0.1", 18557)
            .with_stream_path("/test")
            .with_source(source);

        let session = RuntimeSessionHandle::start(&config).expect("runtime start should succeed");

        assert!(matches!(
            session.poll_event(Duration::from_millis(100)),
            Some(StreamEvent::Started { .. })
        ));

        session.stop().expect("runtime stop should succeed");

        assert_eq!(
            session.poll_event(Duration::from_millis(100)),
            Some(StreamEvent::Stopped { reason: None })
        );
    }

    #[test]
    fn stop_is_idempotent_and_does_not_emit_duplicate_stopped_events() {
        let Some(source) = development_source() else {
            return;
        };

        let config = StreamConfig::new("127.0.0.1", 18558)
            .with_stream_path("/test")
            .with_source(source);

        let session = RuntimeSessionHandle::start(&config).expect("runtime start should succeed");

        assert!(matches!(
            session.poll_event(Duration::from_millis(100)),
            Some(StreamEvent::Started { .. })
        ));

        session.stop().expect("first runtime stop should succeed");
        assert_eq!(
            session.poll_event(Duration::from_millis(100)),
            Some(StreamEvent::Stopped { reason: None })
        );

        session.stop().expect("second runtime stop should succeed");
        assert_eq!(session.poll_event(Duration::from_millis(20)), None);
    }
}
