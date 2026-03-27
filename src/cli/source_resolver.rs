use std::fs;

use crate::cli::args::CliOptions;
use crate::cli::probe::{
    ProbeOutcome, probe_imx500_source, probe_libcamera_source, probe_v4l2_source,
    probe_videotest_source,
};
use raspi_stream::{
    Imx500Source, Imx500Tuning, LibcameraSource, StreamConfig, StreamSource, V4l2Source,
    VideoTestSource,
};

pub(crate) struct ResolvedConfig {
    pub(crate) config: StreamConfig,
    pub(crate) source_label: String,
}

pub(crate) fn build_config(options: &CliOptions) -> Result<ResolvedConfig, String> {
    let (source, resolved_source) = match options.source.as_str() {
        "auto" => detect_default_source(options)?,
        "imx500" => {
            let source = build_imx500_source(options);
            (StreamSource::Imx500(source), "imx500".to_string())
        }
        "libcamera" => {
            let mut source = LibcameraSource::new();
            if let Some(camera_name) = &options.camera_name {
                source = source.with_camera_name(camera_name.clone());
            }
            (StreamSource::Libcamera(source), "libcamera".to_string())
        }
        "v4l2" => resolve_requested_v4l2_source(options)?,
        "videotest" => {
            let mut source = VideoTestSource::new();
            if let Some(pattern) = &options.pattern {
                source = source.with_pattern(pattern.clone());
            }
            (StreamSource::VideoTest(source), "videotest".to_string())
        }
        other => {
            return Err(format!(
                "unsupported source: {other}. expected auto, imx500, libcamera, v4l2 or videotest"
            ));
        }
    };

    Ok(ResolvedConfig {
        config: StreamConfig::new(options.host.clone(), options.port)
            .with_stream_path(options.path.clone())
            .with_source(source)
            .with_resolution(options.width, options.height)
            .with_framerate(options.framerate)
            .with_bitrate(options.bitrate),
        source_label: resolved_source,
    })
}

fn detect_default_source(options: &CliOptions) -> Result<(StreamSource, String), String> {
    let mut probe_diagnostics = Vec::new();

    if options.camera_name.is_some() && options.device_path.is_some() {
        return Err(
            "--source auto では --camera-name と --device-path を同時に使えません".to_string(),
        );
    }

    if options.device_path.is_some() && has_imx500_tuning_overrides(options) {
        return Err(
            "--source auto では IMX500 tuning option と --device-path を同時に使えません"
                .to_string(),
        );
    }

    if has_imx500_tuning_overrides(options) {
        let source = build_imx500_source(options);

        match probe_imx500_source(&source)? {
            ProbeOutcome::Usable => {
                return Ok((
                    StreamSource::Imx500(source),
                    "imx500(auto tuned)".to_string(),
                ));
            }
            ProbeOutcome::Diagnostic(message) => return Err(message),
        }
    }

    if let Some(camera_name) = &options.camera_name {
        let source = LibcameraSource::new().with_camera_name(camera_name.clone());

        match probe_libcamera_source(&source)? {
            ProbeOutcome::Usable => {
                return Ok((
                    StreamSource::Libcamera(source),
                    "libcamera(auto)".to_string(),
                ));
            }
            ProbeOutcome::Diagnostic(message) => return Err(message),
        }
    }

    if let Some(device_path) = &options.device_path {
        let source = V4l2Source::new().with_device_path(device_path.clone());

        match probe_v4l2_source(&source)? {
            ProbeOutcome::Usable => {
                return Ok((
                    StreamSource::V4l2(source),
                    format!("v4l2(auto: {device_path})"),
                ));
            }
            ProbeOutcome::Diagnostic(message) => return Err(message),
        }
    }

    let imx500 = Imx500Source::new();
    match probe_imx500_source(&imx500)? {
        ProbeOutcome::Usable => {
            return Ok((StreamSource::Imx500(imx500), "imx500(auto)".to_string()));
        }
        ProbeOutcome::Diagnostic(message) => probe_diagnostics.push(message),
    }

    let libcamera = LibcameraSource::new();
    match probe_libcamera_source(&libcamera)? {
        ProbeOutcome::Usable => {
            return Ok((
                StreamSource::Libcamera(libcamera),
                "libcamera(auto)".to_string(),
            ));
        }
        ProbeOutcome::Diagnostic(message) => probe_diagnostics.push(message),
    }

    if let Some((source, device_path)) =
        detect_v4l2_device_from_paths(list_v4l2_device_paths()?, probe_v4l2_source)?
    {
        return Ok((
            StreamSource::V4l2(source),
            format!("v4l2(auto: {device_path})"),
        ));
    }

    let mut source = VideoTestSource::new();
    if let Some(pattern) = &options.pattern {
        source = source.with_pattern(pattern.clone());
    }

    match probe_videotest_source(&source)? {
        ProbeOutcome::Usable => {
            return Ok((
                StreamSource::VideoTest(source),
                "videotest(auto fallback)".to_string(),
            ));
        }
        ProbeOutcome::Diagnostic(message) => probe_diagnostics.push(message),
    }

    let diagnostic_suffix = if probe_diagnostics.is_empty() {
        String::new()
    } else {
        format!(" diagnostics: {}", probe_diagnostics.join(" | "))
    };

    Err(format!(
        "no usable input device was detected. tried imx500, libcamera, v4l2, then videotest, but videotestsrc was also unavailable{diagnostic_suffix}"
    ))
}

fn build_imx500_source(options: &CliOptions) -> Imx500Source {
    let mut source = Imx500Source::new();
    if let Some(camera_name) = &options.camera_name {
        source = source.with_camera_name(camera_name.clone());
    }

    let tuning = build_imx500_tuning(options);
    if tuning != Imx500Tuning::new() {
        source = source.with_tuning(tuning);
    }

    source
}

fn build_imx500_tuning(options: &CliOptions) -> Imx500Tuning {
    let mut tuning = Imx500Tuning::new();

    if let Some(exposure_time_us) = options.exposure_time_us {
        tuning = tuning.with_exposure_time_us(exposure_time_us);
    }
    if let Some(analogue_gain) = options.analogue_gain {
        tuning = tuning.with_analogue_gain(analogue_gain);
    }
    if let Some(brightness) = options.brightness {
        tuning = tuning.with_brightness(brightness);
    }
    if let Some(contrast) = options.contrast {
        tuning = tuning.with_contrast(contrast);
    }
    if let Some(saturation) = options.saturation {
        tuning = tuning.with_saturation(saturation);
    }
    if let Some(sharpness) = options.sharpness {
        tuning = tuning.with_sharpness(sharpness);
    }

    tuning
}

fn has_imx500_tuning_overrides(options: &CliOptions) -> bool {
    options.exposure_time_us.is_some()
        || options.analogue_gain.is_some()
        || options.brightness.is_some()
        || options.contrast.is_some()
        || options.saturation.is_some()
        || options.sharpness.is_some()
}

fn resolve_requested_v4l2_source(options: &CliOptions) -> Result<(StreamSource, String), String> {
    if let Some(device_path) = &options.device_path {
        let source = V4l2Source::new().with_device_path(device_path.clone());

        return match probe_v4l2_source(&source)? {
            ProbeOutcome::Usable => {
                Ok((StreamSource::V4l2(source), format!("v4l2 ({device_path})")))
            }
            ProbeOutcome::Diagnostic(message) => Err(message),
        };
    }

    match detect_v4l2_device_from_paths(list_v4l2_device_paths()?, probe_v4l2_source)? {
        Some((source, device_path)) => Ok((
            StreamSource::V4l2(source),
            format!("v4l2(auto: {device_path})"),
        )),
        None => Err(
            "no usable V4L2 device was detected. inspect available nodes with `v4l2-ctl --list-devices`, then retry with `--device-path /dev/videoX`".to_string(),
        ),
    }
}

fn detect_v4l2_device_from_paths<F>(
    device_paths: Vec<String>,
    mut probe: F,
) -> Result<Option<(V4l2Source, String)>, String>
where
    F: FnMut(&V4l2Source) -> Result<ProbeOutcome, String>,
{
    for device_path in device_paths {
        let source = V4l2Source::new().with_device_path(device_path.clone());
        if matches!(probe(&source)?, ProbeOutcome::Usable) {
            return Ok(Some((source, device_path)));
        }
    }

    Ok(None)
}

fn list_v4l2_device_paths() -> Result<Vec<String>, String> {
    let entries = fs::read_dir("/dev")
        .map_err(|error| format!("failed to scan /dev for v4l2 devices: {error}"))?;

    let mut device_paths = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            if !file_name.starts_with("video") {
                return None;
            }

            Some(entry.path().display().to_string())
        })
        .collect::<Vec<_>>();

    sort_v4l2_device_paths(&mut device_paths);
    Ok(device_paths)
}

fn sort_v4l2_device_paths(device_paths: &mut [String]) {
    device_paths.sort_by(|left, right| {
        v4l2_device_sort_key(left)
            .cmp(&v4l2_device_sort_key(right))
            .then_with(|| left.cmp(right))
    });
}

fn v4l2_device_sort_key(device_path: &str) -> (u8, u32) {
    let file_name = device_path.rsplit('/').next().unwrap_or(device_path);
    let suffix = file_name.strip_prefix("video").unwrap_or(file_name);

    match suffix.parse::<u32>() {
        Ok(index) => (0, index),
        Err(_) => (1, u32::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProbeOutcome, ResolvedConfig, build_config, build_imx500_source, build_imx500_tuning,
        detect_default_source, detect_v4l2_device_from_paths, has_imx500_tuning_overrides,
        sort_v4l2_device_paths,
    };
    use crate::cli::args::CliOptions;
    use raspi_stream::{Imx500Source, Imx500Tuning, StreamConfig, StreamSource, V4l2Source};

    #[test]
    fn has_imx500_tuning_overrides_detects_any_override() {
        let options = CliOptions {
            contrast: Some(1.1),
            ..CliOptions::default()
        };

        assert!(has_imx500_tuning_overrides(&options));
    }

    #[test]
    fn build_imx500_tuning_collects_cli_values() {
        let options = CliOptions {
            exposure_time_us: Some(10_000),
            analogue_gain: Some(2.0),
            brightness: Some(0.1),
            contrast: Some(1.1),
            saturation: Some(1.2),
            sharpness: Some(0.8),
            ..CliOptions::default()
        };

        let tuning = build_imx500_tuning(&options);

        assert_eq!(
            tuning,
            Imx500Tuning::new()
                .with_exposure_time_us(10_000)
                .with_analogue_gain(2.0)
                .with_brightness(0.1)
                .with_contrast(1.1)
                .with_saturation(1.2)
                .with_sharpness(0.8)
        );
    }

    #[test]
    fn build_imx500_source_wraps_camera_name_and_tuning() {
        let options = CliOptions {
            camera_name: Some("imx500-main".to_string()),
            exposure_time_us: Some(10_000),
            analogue_gain: Some(2.0),
            ..CliOptions::default()
        };

        let source = build_imx500_source(&options);

        assert_eq!(
            source,
            Imx500Source::new()
                .with_camera_name("imx500-main")
                .with_tuning(
                    Imx500Tuning::new()
                        .with_exposure_time_us(10_000)
                        .with_analogue_gain(2.0),
                )
        );
    }

    #[test]
    fn build_config_applies_imx500_tuning_for_imx500_source() {
        let options = CliOptions {
            source: "imx500".to_string(),
            exposure_time_us: Some(10_000),
            analogue_gain: Some(2.0),
            ..CliOptions::default()
        };

        let ResolvedConfig {
            config,
            source_label,
        } = build_config(&options).expect("build_config should work");

        assert_eq!(source_label, "imx500");
        assert_eq!(
            config,
            StreamConfig::new("127.0.0.1", 8554).with_source(StreamSource::Imx500(
                Imx500Source::new().with_tuning(
                    Imx500Tuning::new()
                        .with_exposure_time_us(10_000)
                        .with_analogue_gain(2.0),
                ),
            ))
        );
    }

    #[test]
    fn detect_default_source_rejects_camera_name_and_device_path_combination() {
        let options = CliOptions {
            camera_name: Some("imx500".to_string()),
            device_path: Some("/dev/video0".to_string()),
            ..CliOptions::default()
        };

        assert_eq!(
            detect_default_source(&options),
            Err("--source auto では --camera-name と --device-path を同時に使えません".to_string())
        );
    }

    #[test]
    fn detect_default_source_rejects_imx500_tuning_and_device_path_combination() {
        let options = CliOptions {
            exposure_time_us: Some(10_000),
            device_path: Some("/dev/video0".to_string()),
            ..CliOptions::default()
        };

        assert_eq!(
            detect_default_source(&options),
            Err(
                "--source auto では IMX500 tuning option と --device-path を同時に使えません"
                    .to_string()
            )
        );
    }

    #[test]
    fn probe_outcome_diagnostic_can_be_matched_by_resolver_logic() {
        let outcome = ProbeOutcome::Diagnostic("camera lookup failed".to_string());

        assert!(matches!(
            outcome,
            ProbeOutcome::Diagnostic(message) if message == "camera lookup failed"
        ));
    }

    #[test]
    fn detect_v4l2_device_from_paths_returns_first_usable_device() {
        let mut probe_targets = Vec::new();
        let device_paths = vec!["/dev/video0".to_string(), "/dev/video2".to_string()];

        let resolved = detect_v4l2_device_from_paths(device_paths, |source| {
            probe_targets.push(source.device_path().unwrap().to_string());

            if source.device_path() == Some("/dev/video2") {
                Ok(ProbeOutcome::Usable)
            } else {
                Ok(ProbeOutcome::Diagnostic("not usable".to_string()))
            }
        })
        .expect("device detection should succeed");

        assert_eq!(
            resolved,
            Some((
                V4l2Source::new().with_device_path("/dev/video2"),
                "/dev/video2".to_string()
            ))
        );
        assert_eq!(probe_targets, vec!["/dev/video0", "/dev/video2"]);
    }

    #[test]
    fn detect_v4l2_device_from_paths_returns_none_when_all_devices_fail() {
        let resolved = detect_v4l2_device_from_paths(
            vec!["/dev/video0".to_string(), "/dev/video2".to_string()],
            |_| Ok(ProbeOutcome::Diagnostic("not usable".to_string())),
        )
        .expect("device detection should finish without probe errors");

        assert_eq!(resolved, None);
    }

    #[test]
    fn sort_v4l2_device_paths_orders_numeric_suffixes_numerically() {
        let mut device_paths = vec![
            "/dev/video14".to_string(),
            "/dev/video2".to_string(),
            "/dev/video10".to_string(),
            "/dev/video1".to_string(),
        ];

        sort_v4l2_device_paths(&mut device_paths);

        assert_eq!(
            device_paths,
            vec![
                "/dev/video1".to_string(),
                "/dev/video2".to_string(),
                "/dev/video10".to_string(),
                "/dev/video14".to_string(),
            ]
        );
    }
}
