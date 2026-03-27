use gstreamer as gst;

use crate::{
    Imx500Source, LibcameraSource, StartupDiagnostic, StreamConfig, StreamError, StreamSource,
    V4l2Source, VideoTestSource,
    gst_support::{
        DEFAULT_PROBE_TIMEOUT_MS, LaunchPipelineProbe, ProbeBuildError, ProbeSourceKind,
        StartupProbeContext, format_startup_probe_build_error, format_startup_probe_failure,
        format_startup_probe_start_error,
    },
    pipeline,
};

pub(super) struct SelectedPipeline {
    pub(super) label: &'static str,
    pub(super) description: String,
    pub(super) startup_diagnostics: Vec<StartupDiagnostic>,
}

pub(super) fn select_pipeline(config: &StreamConfig) -> Result<SelectedPipeline, StreamError> {
    let candidates = pipeline::build_stream_pipeline_candidates(config);
    let mut errors = Vec::new();

    for candidate in candidates {
        match preflight_pipeline(config, &candidate.description) {
            Ok(()) => {
                return Ok(SelectedPipeline {
                    label: candidate.label,
                    description: candidate.description,
                    startup_diagnostics: errors
                        .into_iter()
                        .map(|(label, message)| StartupDiagnostic::new(label, message))
                        .collect(),
                });
            }
            Err(error) => errors.push((candidate.label, stream_error_message(error))),
        }
    }

    let probe_target = startup_probe_target(config.source());
    let details = summarize_candidate_errors(errors);

    Err(StreamError::RuntimeError(format!(
        "failed to find a usable pipeline for {probe_target}. {details}"
    )))
}

fn summarize_candidate_errors(errors: Vec<(&'static str, String)>) -> String {
    let mut grouped: Vec<(String, Vec<&'static str>)> = Vec::new();

    for (label, message) in errors {
        if let Some((_, labels)) = grouped
            .iter_mut()
            .find(|(existing_message, _)| *existing_message == message)
        {
            labels.push(label);
            continue;
        }

        grouped.push((message, vec![label]));
    }

    grouped
        .into_iter()
        .map(|(message, labels)| format!("{}: {message}", labels.join(", ")))
        .collect::<Vec<_>>()
        .join(" | ")
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

fn preflight_pipeline(
    config: &StreamConfig,
    pipeline_description: &str,
) -> Result<(), StreamError> {
    let probe_target = startup_probe_target(config.source());
    let probe_hint = startup_probe_hint(config.source());
    let probe_context = StartupProbeContext {
        target: &probe_target,
        source_kind: probe_source_kind(config.source()),
        hint: probe_hint.as_deref(),
        v4l2_device_path: v4l2_device_path(config.source()),
    };
    let probe_description = format!("{pipeline_description} ! fakesink sync=false async=false");

    let probe = LaunchPipelineProbe::from_launch(&probe_description)
        .map_err(|error| map_probe_build_error(error, &probe_context))?;

    if let Err(error) = probe.start() {
        let detailed_error = startup_probe_error_from_bus(probe.bus(), &probe_context);
        probe.shutdown();
        return Err(match detailed_error {
            Some(message) => StreamError::RuntimeError(message),
            None => StreamError::StateChangeFailed(format_startup_probe_start_error(
                &probe_context,
                &error,
            )),
        });
    }

    let message = probe.timed_pop_filtered(
        DEFAULT_PROBE_TIMEOUT_MS,
        &[
            gst::MessageType::Error,
            gst::MessageType::AsyncDone,
            gst::MessageType::Eos,
        ],
    );

    let result = match message {
        Some(message) => match message.view() {
            gst::MessageView::Error(error) => {
                Err(StreamError::RuntimeError(format_startup_probe_failure(
                    &probe_context,
                    &error.error().to_string(),
                    error.debug().as_ref().map(|debug| debug.as_str()),
                )))
            }
            _ => Ok(()),
        },
        None => Ok(()),
    };

    probe.shutdown();

    result
}

fn map_probe_build_error(error: ProbeBuildError, context: &StartupProbeContext<'_>) -> StreamError {
    match error {
        ProbeBuildError::MissingBus => {
            StreamError::RuntimeError(format_startup_probe_build_error(context, error))
        }
        _ => StreamError::PipelineBuildFailed(format_startup_probe_build_error(context, error)),
    }
}

fn startup_probe_error_from_bus(
    bus: &gst::Bus,
    context: &StartupProbeContext<'_>,
) -> Option<String> {
    let message = bus.timed_pop_filtered(
        gst::ClockTime::from_mseconds(DEFAULT_PROBE_TIMEOUT_MS),
        &[gst::MessageType::Error],
    )?;

    match message.view() {
        gst::MessageView::Error(error) => Some(format_startup_probe_failure(
            context,
            &error.error().to_string(),
            error.debug().as_ref().map(|debug| debug.as_str()),
        )),
        _ => None,
    }
}

fn startup_probe_target(source: &StreamSource) -> String {
    match source {
        StreamSource::Imx500(source) => imx500_probe_target(source),
        StreamSource::Libcamera(source) => libcamera_probe_target(source),
        StreamSource::V4l2(source) => v4l2_probe_target(source),
        StreamSource::VideoTest(source) => videotest_probe_target(source),
    }
}

fn startup_probe_hint(source: &StreamSource) -> Option<String> {
    match source {
        StreamSource::Imx500(_) => Some(
            "verify the IMX500 camera is connected, libcamerasrc can open it, and the downstream H.264 elements are available".to_string(),
        ),
        StreamSource::Libcamera(_) => Some(
            "verify a libcamera-compatible device is connected and the downstream H.264 elements are available".to_string(),
        ),
        StreamSource::V4l2(_) => Some("verify the V4L2 device exists and is not busy".to_string()),
        StreamSource::VideoTest(_) => Some(
            "verify the pattern name and required GStreamer elements are available".to_string(),
        ),
    }
}

fn imx500_probe_target(source: &Imx500Source) -> String {
    match source.camera_name() {
        Some(camera_name) => format!("imx500 camera \"{camera_name}\""),
        None => "default imx500 camera".to_string(),
    }
}

fn libcamera_probe_target(source: &LibcameraSource) -> String {
    match source.camera_name() {
        Some(camera_name) => format!("libcamera camera \"{camera_name}\""),
        None => "default libcamera camera".to_string(),
    }
}

fn v4l2_probe_target(source: &V4l2Source) -> String {
    match source.device_path() {
        Some(device_path) => format!("v4l2 device \"{device_path}\""),
        None => "default v4l2 device".to_string(),
    }
}

fn videotest_probe_target(source: &VideoTestSource) -> String {
    match source.pattern() {
        Some(pattern) => format!("videotest pattern \"{pattern}\""),
        None => "videotest source".to_string(),
    }
}

fn probe_source_kind(source: &StreamSource) -> ProbeSourceKind {
    match source {
        StreamSource::Imx500(_) => ProbeSourceKind::Imx500,
        StreamSource::Libcamera(_) => ProbeSourceKind::Libcamera,
        StreamSource::V4l2(_) => ProbeSourceKind::V4l2,
        StreamSource::VideoTest(_) => ProbeSourceKind::VideoTest,
    }
}

fn v4l2_device_path(source: &StreamSource) -> Option<&str> {
    match source {
        StreamSource::V4l2(source) => source.device_path(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{probe_source_kind, summarize_candidate_errors, v4l2_device_path};
    use crate::gst_support::format_probe_resolution;
    use crate::{Imx500Source, LibcameraSource, StreamSource, V4l2Source};

    #[test]
    fn missing_libcamerasrc_adds_install_hint_for_libcamera_sources() {
        let source = StreamSource::Libcamera(LibcameraSource::new());
        let resolution = format_probe_resolution(
            probe_source_kind(&source),
            "no element \"libcamerasrc\"",
            v4l2_device_path(&source),
        );

        assert!(resolution.contains("gstreamer1.0-libcamera"));
        assert!(resolution.contains("gst-inspect-1.0 libcamerasrc"));
    }

    #[test]
    fn v4l2_missing_element_does_not_add_libcamera_install_hint() {
        let source = StreamSource::V4l2(V4l2Source::new());
        let resolution = format_probe_resolution(
            probe_source_kind(&source),
            "no element \"v4l2src\"",
            v4l2_device_path(&source),
        );

        assert!(!resolution.contains("gstreamer1.0-libcamera"));
        assert!(resolution.contains("gstreamer1.0-plugins-good"));
    }

    #[test]
    fn missing_h264_elements_add_probe_hint_for_rpi_camera_sources() {
        let source = StreamSource::Imx500(Imx500Source::new());
        let resolution = format_probe_resolution(
            probe_source_kind(&source),
            "no element \"v4l2h264enc\"",
            v4l2_device_path(&source),
        );

        assert!(resolution.contains("gst-inspect-1.0 v4l2h264enc h264parse rtph264pay"));
    }

    #[test]
    fn missing_x264enc_adds_install_hint() {
        let source = StreamSource::Imx500(Imx500Source::new());
        let resolution = format_probe_resolution(
            probe_source_kind(&source),
            "no element \"x264enc\"",
            v4l2_device_path(&source),
        );

        assert!(resolution.contains("gstreamer1.0-plugins-ugly"));
        assert!(resolution.contains("gst-inspect-1.0 x264enc"));
    }

    #[test]
    fn v4l2_encoder_frame_failure_adds_fallback_hint() {
        let source = StreamSource::Imx500(Imx500Source::new());
        let resolution = format_probe_resolution(
            probe_source_kind(&source),
            "Failed to process frame.",
            v4l2_device_path(&source),
        );

        assert!(resolution.contains("x264enc"));
    }

    #[test]
    fn v4l2_not_negotiated_adds_supported_modes_hint() {
        let source = StreamSource::V4l2(V4l2Source::new());
        let resolution = format_probe_resolution(
            probe_source_kind(&source),
            "streaming stopped, reason not-negotiated (-4)",
            v4l2_device_path(&source),
        );

        assert!(resolution.contains("v4l2-ctl --device /dev/video0 --list-formats-ext"));
        assert!(resolution.contains("--width --height --framerate"));
    }

    #[test]
    fn v4l2_not_negotiated_uses_configured_device_path() {
        let source = StreamSource::V4l2(V4l2Source::new().with_device_path("/dev/video2"));
        let resolution = format_probe_resolution(
            probe_source_kind(&source),
            "streaming stopped, reason not-negotiated (-4)",
            v4l2_device_path(&source),
        );

        assert!(resolution.contains("v4l2-ctl --device /dev/video2 --list-formats-ext"));
    }

    #[test]
    fn summarize_candidate_errors_groups_identical_messages() {
        let summary = summarize_candidate_errors(vec![
            ("v4l2h264enc/raw-direct", "same failure".to_string()),
            ("x264enc/raw-direct", "same failure".to_string()),
            ("v4l2h264enc/mjpeg", "other failure".to_string()),
        ]);

        assert!(summary.contains("v4l2h264enc/raw-direct, x264enc/raw-direct: same failure"));
        assert!(summary.contains("v4l2h264enc/mjpeg: other failure"));
    }
}
