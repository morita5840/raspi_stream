use std::sync::mpsc;

use gstreamer as gst;
use gstreamer::prelude::*;

use crate::StreamEvent;

const RTSP_SERVER_SOURCE: &str = "rtsp-server";

pub(super) fn stream_event_from_message(message: &gst::Message) -> Option<StreamEvent> {
    match message.view() {
        gst::MessageView::Warning(warning) => Some(StreamEvent::Warning {
            source: message_source_name(message),
            message: format!(
                "{}{}",
                warning.error(),
                debug_suffix(warning.debug().as_ref().map(|debug| debug.as_str()))
            ),
        }),
        gst::MessageView::Error(error) => Some(StreamEvent::Error {
            source: message_source_name(message),
            message: format!(
                "{}{}",
                error.error(),
                debug_suffix(error.debug().as_ref().map(|debug| debug.as_str()))
            ),
        }),
        gst::MessageView::Eos(..) => Some(StreamEvent::EndOfStream),
        _ => None,
    }
}

pub(super) fn emit_runtime_failure_events(event_tx: &mpsc::Sender<StreamEvent>, message: String) {
    let _ = event_tx.send(StreamEvent::Error {
        source: RTSP_SERVER_SOURCE.to_string(),
        message: message.clone(),
    });
    let _ = event_tx.send(StreamEvent::Stopped {
        reason: Some(message),
    });
}

fn message_source_name(message: &gst::Message) -> String {
    message
        .src()
        .map(|source| source.path_string().to_string())
        .unwrap_or_else(|| "gstreamer".to_string())
}

fn debug_suffix(debug: Option<&str>) -> String {
    match debug {
        Some(debug) if !debug.is_empty() => format!(" ({debug})"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::{emit_runtime_failure_events, stream_event_from_message};
    use crate::StreamEvent;
    use gstreamer as gst;

    #[test]
    fn warning_message_maps_to_warning_event() {
        gst::init().expect("gstreamer init should succeed");

        let pipeline = gst::Pipeline::with_name("warning-pipeline");
        let message = gst::message::Warning::builder(gst::CoreError::Failed, "warning message")
            .src(&pipeline)
            .debug("warning debug")
            .build();

        assert_eq!(
            stream_event_from_message(&message),
            Some(StreamEvent::Warning {
                source: String::from("/GstPipeline:warning-pipeline"),
                message: String::from("warning message (warning debug)"),
            })
        );
    }

    #[test]
    fn error_message_maps_to_error_event() {
        gst::init().expect("gstreamer init should succeed");

        let pipeline = gst::Pipeline::with_name("error-pipeline");
        let message = gst::message::Error::builder(gst::CoreError::Failed, "error message")
            .src(&pipeline)
            .debug("error debug")
            .build();

        assert_eq!(
            stream_event_from_message(&message),
            Some(StreamEvent::Error {
                source: String::from("/GstPipeline:error-pipeline"),
                message: String::from("error message (error debug)"),
            })
        );
    }

    #[test]
    fn eos_message_maps_to_end_of_stream_event() {
        gst::init().expect("gstreamer init should succeed");

        let pipeline = gst::Pipeline::with_name("eos-pipeline");
        let message = gst::message::Eos::builder().src(&pipeline).build();

        assert_eq!(
            stream_event_from_message(&message),
            Some(StreamEvent::EndOfStream)
        );
    }

    #[test]
    fn runtime_failure_emits_error_then_stopped() {
        let (event_tx, event_rx) = mpsc::channel();

        emit_runtime_failure_events(&event_tx, String::from("runtime failure"));

        assert_eq!(
            event_rx.recv().ok(),
            Some(StreamEvent::Error {
                source: String::from("rtsp-server"),
                message: String::from("runtime failure"),
            })
        );
        assert_eq!(
            event_rx.recv().ok(),
            Some(StreamEvent::Stopped {
                reason: Some(String::from("runtime failure")),
            })
        );
    }
}
