use std::net::{IpAddr, UdpSocket};

use crate::cli::args::CliOptions;
use raspi_stream::{Imx500Source, StartupDiagnostic, StreamConfig, StreamSource};

pub(crate) fn startup_summary_lines(
    options: &CliOptions,
    config: &StreamConfig,
    resolved_source: &str,
) -> Vec<String> {
    let mut lines = vec![
        format!("  bind host : {}", options.host),
        format!("  port      : {}", options.port),
        format!("  path      : {}", options.path),
        format!("  source    : {resolved_source}"),
        format!(
            "  bind url  : rtsp://{}:{}{}",
            options.host, options.port, options.path
        ),
        format!("  resolution: {}x{}", options.width, options.height),
        format!("  fps       : {}", options.framerate),
        format!("  bitrate   : {}", options.bitrate),
    ];

    lines.extend(client_access_summary_lines(
        &options.host,
        options.port,
        &options.path,
        infer_client_access_host(&options.host),
    ));

    if let StreamSource::Imx500(source) = config.source() {
        lines.extend(imx500_tuning_summary_lines(source));
    }

    lines
}

pub(crate) fn encoder_summary_line(pipeline_label: &str) -> String {
    format!("  encoder   : {pipeline_label}")
}

pub(crate) fn startup_diagnostic_note_lines(diagnostics: &[StartupDiagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| format!("  note      : {}", diagnostic.summary_line()))
        .collect()
}

fn imx500_tuning_summary_lines(source: &Imx500Source) -> Vec<String> {
    let tuning = source.tuning();
    let mut lines = vec![match source.camera_name() {
        Some(camera_name) => format!("  camera    : {camera_name}"),
        None => "  camera    : <default>".to_string(),
    }];

    if let Some(exposure_time_us) = tuning.exposure_time_us() {
        lines.push(format!("  exposure  : {exposure_time_us} us"));
    }
    if let Some(analogue_gain) = tuning.analogue_gain() {
        lines.push(format!("  gain      : {analogue_gain}"));
    }
    if let Some(brightness) = tuning.brightness() {
        lines.push(format!("  brightness: {brightness}"));
    }
    if let Some(contrast) = tuning.contrast() {
        lines.push(format!("  contrast  : {contrast}"));
    }
    if let Some(saturation) = tuning.saturation() {
        lines.push(format!("  saturation: {saturation}"));
    }
    if let Some(sharpness) = tuning.sharpness() {
        lines.push(format!("  sharpness : {sharpness}"));
    }

    lines
}

fn client_access_summary_lines(
    host: &str,
    port: u16,
    path: &str,
    detected_host: Option<String>,
) -> Vec<String> {
    match host {
        "0.0.0.0" => match detected_host {
            Some(detected_host) => vec![format!("  client url: rtsp://{detected_host}:{port}{path}")],
            None => vec![
                format!("  client url: rtsp://<raspberry-pi-ip>:{port}{path}"),
                "  note      : replace <raspberry-pi-ip> with an address reachable from the client"
                    .to_string(),
            ],
        },
        "::" => match detected_host {
            Some(detected_host) => vec![format!("  client url: rtsp://[{detected_host}]:{port}{path}")],
            None => vec![
                format!("  client url: rtsp://[<raspberry-pi-ipv6>]:{port}{path}"),
                "  note      : replace <raspberry-pi-ipv6> with an address reachable from the client"
                    .to_string(),
            ],
        },
        "127.0.0.1" | "localhost" => {
            vec!["  access    : local machine only".to_string()]
        }
        _ => vec![format!("  client url: rtsp://{host}:{port}{path}")],
    }
}

fn infer_client_access_host(bind_host: &str) -> Option<String> {
    match bind_host {
        "0.0.0.0" => infer_local_ip("0.0.0.0:0", "192.0.2.1:80"),
        "::" => infer_local_ip("[::]:0", "[2001:db8::1]:80"),
        _ => None,
    }
}

fn infer_local_ip(bind_addr: &str, probe_addr: &str) -> Option<String> {
    let socket = UdpSocket::bind(bind_addr).ok()?;
    socket.connect(probe_addr).ok()?;

    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(address) if !address.is_loopback() => Some(address.to_string()),
        IpAddr::V6(address) if !address.is_loopback() => Some(address.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        client_access_summary_lines, encoder_summary_line, imx500_tuning_summary_lines,
        infer_client_access_host, startup_diagnostic_note_lines, startup_summary_lines,
    };
    use crate::cli::args::CliOptions;
    use raspi_stream::{Imx500Source, Imx500Tuning, StartupDiagnostic, StreamConfig, StreamSource};

    #[test]
    fn imx500_tuning_summary_lines_include_only_set_values() {
        let lines = imx500_tuning_summary_lines(
            &Imx500Source::new().with_tuning(
                Imx500Tuning::new()
                    .with_exposure_time_us(10_000)
                    .with_analogue_gain(2.0)
                    .with_contrast(1.1),
            ),
        );

        assert_eq!(
            lines,
            vec![
                "  camera    : <default>".to_string(),
                "  exposure  : 10000 us".to_string(),
                "  gain      : 2".to_string(),
                "  contrast  : 1.1".to_string(),
            ]
        );
    }

    #[test]
    fn startup_summary_lines_append_imx500_tuning_lines() {
        let options = CliOptions {
            source: "imx500".to_string(),
            exposure_time_us: Some(10_000),
            ..CliOptions::default()
        };
        let config = StreamConfig::new("127.0.0.1", 8554).with_source(StreamSource::Imx500(
            Imx500Source::new().with_tuning(Imx500Tuning::new().with_exposure_time_us(10_000)),
        ));

        let lines = startup_summary_lines(&options, &config, "imx500");

        assert!(lines.contains(&"  source    : imx500".to_string()));
        assert!(lines.contains(&"  camera    : <default>".to_string()));
        assert!(lines.contains(&"  exposure  : 10000 us".to_string()));
        assert!(lines.contains(&"  access    : local machine only".to_string()));
    }

    #[test]
    fn startup_summary_lines_show_client_url_hint_for_wildcard_bind() {
        let lines = client_access_summary_lines("0.0.0.0", 8554, "/camera", None);

        assert_eq!(
            lines,
            vec![
                "  client url: rtsp://<raspberry-pi-ip>:8554/camera".to_string(),
                "  note      : replace <raspberry-pi-ip> with an address reachable from the client"
                    .to_string(),
            ]
        );
    }

    #[test]
    fn startup_summary_lines_show_client_url_for_specific_host() {
        let options = CliOptions {
            host: "192.168.1.50".to_string(),
            path: "/camera".to_string(),
            ..CliOptions::default()
        };
        let config = StreamConfig::new("192.168.1.50", 8554).with_source(StreamSource::videotest());

        let lines = startup_summary_lines(&options, &config, "videotest");

        assert!(lines.contains(&"  client url: rtsp://192.168.1.50:8554/camera".to_string()));
    }

    #[test]
    fn client_access_summary_lines_use_detected_ipv4_for_wildcard_bind() {
        let lines = client_access_summary_lines(
            "0.0.0.0",
            8554,
            "/camera",
            Some("192.168.1.16".to_string()),
        );

        assert_eq!(
            lines,
            vec!["  client url: rtsp://192.168.1.16:8554/camera".to_string()]
        );
    }

    #[test]
    fn infer_client_access_host_returns_none_for_specific_host() {
        assert_eq!(infer_client_access_host("192.168.1.50"), None);
    }

    #[test]
    fn encoder_summary_line_uses_pipeline_label() {
        assert_eq!(
            encoder_summary_line("v4l2h264enc"),
            "  encoder   : v4l2h264enc".to_string()
        );
    }

    #[test]
    fn startup_diagnostic_note_lines_shorten_not_negotiated_messages() {
        let lines = startup_diagnostic_note_lines(&[StartupDiagnostic::new(
            "x264enc/raw-direct",
            "startup probe failed for v4l2 device \"/dev/video2\": Internal data stream error. streaming stopped, reason not-negotiated (-4)",
        )]);

        assert_eq!(
            lines,
            vec!["  note      : startup fallback: skipped x264enc/raw-direct (camera caps were rejected)".to_string()]
        );
    }

    #[test]
    fn startup_diagnostic_note_lines_shorten_encoder_failures() {
        let lines = startup_diagnostic_note_lines(&[StartupDiagnostic::new(
            "v4l2h264enc/raw-convert",
            "startup probe failed for v4l2 device \"/dev/video2\": Failed to process frame.",
        )]);

        assert_eq!(
            lines,
            vec!["  note      : startup fallback: skipped v4l2h264enc/raw-convert (hardware H.264 encoder failed)".to_string()]
        );
    }
}
