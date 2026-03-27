use std::fmt::Display;

use super::{
    ProbeBuildError, ProbeSourceKind, build_probe_search_text, format_probe_guidance,
    format_probe_resolution,
};

pub struct StartupProbeContext<'a> {
    pub target: &'a str,
    pub source_kind: ProbeSourceKind,
    pub hint: Option<&'a str>,
    pub v4l2_device_path: Option<&'a str>,
}

pub fn format_startup_probe_failure(
    context: &StartupProbeContext<'_>,
    error_message: &str,
    debug_text: Option<&str>,
) -> String {
    let search_text = build_probe_search_text(error_message, debug_text);
    let resolution =
        format_probe_resolution(context.source_kind, &search_text, context.v4l2_device_path);
    let guidance = format_probe_guidance(context.hint, &resolution);

    format!(
        "startup probe failed for {}: {}{}{}",
        context.target,
        error_message,
        debug_suffix(debug_text),
        guidance,
    )
}

pub fn format_startup_probe_start_error(
    context: &StartupProbeContext<'_>,
    error: &impl Display,
) -> String {
    format!(
        "failed to start startup probe for {}: {}{}",
        context.target,
        error,
        format_probe_guidance(context.hint, ""),
    )
}

pub fn format_startup_probe_build_error(
    context: &StartupProbeContext<'_>,
    error: ProbeBuildError,
) -> String {
    match error {
        ProbeBuildError::Parse(error) => {
            let error_text = error.to_string();
            let resolution =
                format_probe_resolution(context.source_kind, &error_text, context.v4l2_device_path);
            let guidance = format_probe_guidance(context.hint, &resolution);

            format!(
                "failed to build startup probe for {}: {}{}",
                context.target, error_text, guidance,
            )
        }
        ProbeBuildError::NotPipeline => format!(
            "failed to build startup probe pipeline for {}{}",
            context.target,
            format_probe_guidance(context.hint, "")
        ),
        ProbeBuildError::MissingBus => format!(
            "failed to create startup probe bus for {}{}",
            context.target,
            format_probe_guidance(context.hint, "")
        ),
    }
}

fn debug_suffix(debug: Option<&str>) -> String {
    match debug {
        Some(debug) if !debug.is_empty() => format!(" ({debug})"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StartupProbeContext, format_startup_probe_build_error, format_startup_probe_failure,
    };
    use crate::gst_support::{ProbeBuildError, ProbeSourceKind};
    use gstreamer as gst;

    #[test]
    fn format_startup_probe_failure_uses_contextual_v4l2_path() {
        let message = format_startup_probe_failure(
            &StartupProbeContext {
                target: "v4l2 device \"/dev/video2\"",
                source_kind: ProbeSourceKind::V4l2,
                hint: Some("verify the V4L2 device exists and is not busy"),
                v4l2_device_path: Some("/dev/video2"),
            },
            "Internal data stream error.",
            Some("streaming stopped, reason not-negotiated (-4)"),
        );

        assert!(message.contains("startup probe failed for v4l2 device \"/dev/video2\""));
        assert!(message.contains("v4l2-ctl --device /dev/video2 --list-formats-ext"));
    }

    #[test]
    fn format_startup_probe_build_error_uses_missing_bus_message() {
        let message = format_startup_probe_build_error(
            &StartupProbeContext {
                target: "videotest source",
                source_kind: ProbeSourceKind::VideoTest,
                hint: Some("verify required elements are available"),
                v4l2_device_path: None,
            },
            ProbeBuildError::MissingBus,
        );

        assert!(message.contains("failed to create startup probe bus for videotest source"));
    }

    #[test]
    fn format_startup_probe_build_error_parse_keeps_resolution_guidance() {
        gst::init().expect("gstreamer init should succeed");

        let parse_error = gst::parse::launch("!").expect_err("launch should fail");
        let message = format_startup_probe_build_error(
            &StartupProbeContext {
                target: "v4l2 device \"/dev/video2\"",
                source_kind: ProbeSourceKind::V4l2,
                hint: Some("verify the V4L2 device exists and is not busy"),
                v4l2_device_path: Some("/dev/video2"),
            },
            ProbeBuildError::Parse(parse_error),
        );

        assert!(message.contains("failed to build startup probe for v4l2 device \"/dev/video2\""));
    }
}
