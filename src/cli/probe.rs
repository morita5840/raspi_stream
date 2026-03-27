use gstreamer as gst;

use raspi_stream::gst_support::{
    DEFAULT_PROBE_TIMEOUT_MS, LaunchPipelineProbe, ProbeSourceKind, StartupProbeContext,
    build_libcamerasrc_fragment, build_v4l2src_fragment, build_videotestsrc_fragment,
    format_startup_probe_build_error, format_startup_probe_failure,
    format_startup_probe_start_error,
};
use raspi_stream::{Imx500Source, LibcameraSource, V4l2Source, VideoTestSource};

pub(crate) enum ProbeOutcome {
    Usable,
    Diagnostic(String),
}

pub(crate) fn probe_imx500_source(source: &Imx500Source) -> Result<ProbeOutcome, String> {
    let libcamera_source = match source.camera_name() {
        Some(camera_name) => LibcameraSource::new().with_camera_name(camera_name),
        None => LibcameraSource::new(),
    };

    probe_libcamera_source_with_target(
        &libcamera_source,
        &match source.camera_name() {
            Some(camera_name) => format!("imx500 camera \"{camera_name}\""),
            None => "default imx500 camera".to_string(),
        },
        ProbeSourceKind::Imx500,
        "verify the IMX500 camera is connected and libcamerasrc can open it",
    )
}

pub(crate) fn probe_libcamera_source(source: &LibcameraSource) -> Result<ProbeOutcome, String> {
    let target = match source.camera_name() {
        Some(camera_name) => format!("libcamera camera \"{camera_name}\""),
        None => "default libcamera camera".to_string(),
    };

    probe_libcamera_source_with_target(
        source,
        &target,
        ProbeSourceKind::Libcamera,
        "verify a libcamera-compatible device is connected and libcamerasrc can open it",
    )
}

fn probe_libcamera_source_with_target(
    source: &LibcameraSource,
    target: &str,
    source_kind: ProbeSourceKind,
    hint: &str,
) -> Result<ProbeOutcome, String> {
    let mut description = build_libcamerasrc_fragment(source.camera_name(), Some(1));

    description.push_str(" ! fakesink sync=false async=false");

    probe_pipeline(&description, target, source_kind, Some(hint))
}

pub(crate) fn probe_v4l2_source(source: &V4l2Source) -> Result<ProbeOutcome, String> {
    let mut description = build_v4l2src_fragment(source.device_path(), Some(1));
    let target = match source.device_path() {
        Some(device_path) => format!("v4l2 device \"{device_path}\""),
        None => "default v4l2 device".to_string(),
    };

    description.push_str(" ! fakesink sync=false async=false");

    probe_pipeline(
        &description,
        &target,
        ProbeSourceKind::V4l2,
        Some("verify the V4L2 device exists and is not busy"),
    )
}

pub(crate) fn probe_videotest_source(source: &VideoTestSource) -> Result<ProbeOutcome, String> {
    let mut description = build_videotestsrc_fragment(source.is_live(), source.pattern(), Some(1));
    let target = match source.pattern() {
        Some(pattern) => format!("videotest pattern \"{pattern}\""),
        None => "videotest source".to_string(),
    };

    description.push_str(
        " ! videoconvert ! video/x-raw,format=I420 ! vp8enc deadline=1 cpu-used=8 ! rtpvp8pay pt=96 ! fakesink sync=false async=false",
    );

    probe_pipeline(
        &description,
        &target,
        ProbeSourceKind::VideoTest,
        Some("verify the pattern name and required GStreamer elements are available"),
    )
}

fn probe_pipeline(
    description: &str,
    target: &str,
    source_kind: ProbeSourceKind,
    hint: Option<&str>,
) -> Result<ProbeOutcome, String> {
    gst::init().map_err(|error| format!("failed to initialize gstreamer: {error}"))?;

    let context = StartupProbeContext {
        target,
        source_kind,
        hint,
        v4l2_device_path: v4l2_device_path_from_target(source_kind, target),
    };

    let probe = match LaunchPipelineProbe::from_launch(description) {
        Ok(probe) => probe,
        Err(error) => {
            return Ok(ProbeOutcome::Diagnostic(format_startup_probe_build_error(
                &context, error,
            )));
        }
    };

    if let Err(error) = probe.start() {
        let message = startup_probe_error_from_bus(&probe, &context)
            .unwrap_or_else(|| format_startup_probe_start_error(&context, &error));
        probe.shutdown();
        return Ok(ProbeOutcome::Diagnostic(message));
    }

    let message = probe.timed_pop_filtered(
        DEFAULT_PROBE_TIMEOUT_MS,
        &[
            gst::MessageType::Error,
            gst::MessageType::AsyncDone,
            gst::MessageType::Eos,
        ],
    );

    let outcome = match message {
        Some(message) => match message.view() {
            gst::MessageView::Error(error) => {
                ProbeOutcome::Diagnostic(format_startup_probe_failure(
                    &context,
                    &error.error().to_string(),
                    error.debug().as_ref().map(|debug| debug.as_str()),
                ))
            }
            _ => ProbeOutcome::Usable,
        },
        None => ProbeOutcome::Usable,
    };

    probe.shutdown();

    Ok(outcome)
}

fn startup_probe_error_from_bus(
    probe: &LaunchPipelineProbe,
    context: &StartupProbeContext<'_>,
) -> Option<String> {
    let message = probe.bus().timed_pop_filtered(
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

fn v4l2_device_path_from_target(source_kind: ProbeSourceKind, target: &str) -> Option<&str> {
    if !matches!(source_kind, ProbeSourceKind::V4l2) {
        return None;
    }

    target
        .strip_prefix("v4l2 device \"")
        .and_then(|rest| rest.strip_suffix('"'))
}

#[cfg(test)]
mod tests {
    use super::ProbeOutcome;
    use raspi_stream::gst_support::{ProbeSourceKind, format_probe_resolution};

    #[test]
    fn format_probe_resolution_adds_libcamera_install_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::Libcamera,
            "no element \"libcamerasrc\"",
            None,
        );

        assert!(resolution.contains("gstreamer1.0-libcamera"));
        assert!(resolution.contains("gst-inspect-1.0 libcamerasrc"));
    }

    #[test]
    fn format_probe_resolution_adds_camera_name_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::Libcamera,
            "Could not find a camera named 'main'",
            None,
        );

        assert!(resolution.contains("--camera-name"));
        assert!(resolution.contains("rpicam-hello --list-cameras"));
    }

    #[test]
    fn format_probe_resolution_adds_v4l2_install_hint() {
        let resolution =
            format_probe_resolution(ProbeSourceKind::V4l2, "no element \"v4l2src\"", None);

        assert!(resolution.contains("gstreamer1.0-plugins-good"));
        assert!(resolution.contains("gst-inspect-1.0 v4l2src"));
    }

    #[test]
    fn format_probe_resolution_adds_missing_device_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::V4l2,
            "Could not open device '/dev/video9': No such file or directory",
            Some("/dev/video9"),
        );

        assert!(resolution.contains("--device-path"));
        assert!(resolution.contains("v4l2-ctl --list-devices"));
    }

    #[test]
    fn format_probe_resolution_adds_busy_device_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::V4l2,
            "Device or resource busy",
            Some("/dev/video2"),
        );

        assert!(resolution.contains("fuser /dev/video2"));
        assert!(resolution.contains("lsof /dev/video2"));
    }

    #[test]
    fn format_probe_resolution_adds_not_negotiated_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::V4l2,
            "streaming stopped, reason not-negotiated (-4)",
            Some("/dev/video2"),
        );

        assert!(resolution.contains("v4l2-ctl --device /dev/video2 --list-formats-ext"));
    }

    #[test]
    fn probe_outcome_supports_diagnostic_messages() {
        let outcome = ProbeOutcome::Diagnostic("probe failed".to_string());

        assert!(matches!(outcome, ProbeOutcome::Diagnostic(message) if message == "probe failed"));
    }
}
