mod rpi_camera;
mod v4l2_camera;
mod videotest;

use crate::{StreamConfig, StreamSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PipelineCandidate {
    pub(crate) label: &'static str,
    pub(crate) description: String,
}

#[cfg(test)]
pub(crate) fn build_stream_pipeline(config: &StreamConfig) -> String {
    build_stream_pipeline_candidates(config)
        .into_iter()
        .next()
        .map(|candidate| candidate.description)
        .unwrap_or_default()
}

pub(crate) fn build_stream_pipeline_candidates(config: &StreamConfig) -> Vec<PipelineCandidate> {
    match config.source() {
        StreamSource::Imx500(..) => rpi_camera::build_pipeline_candidates(config),
        StreamSource::Libcamera(..) => rpi_camera::build_pipeline_candidates(config),
        StreamSource::V4l2(..) => v4l2_camera::build_pipeline_candidates(config),
        StreamSource::VideoTest(..) => vec![PipelineCandidate {
            label: "vp8",
            description: videotest::build_pipeline(config),
        }],
    }
}

pub(crate) fn build_h264_rtsp_pipeline_candidates(
    config_source: &str,
    raw_caps: &str,
    config: &StreamConfig,
) -> Vec<PipelineCandidate> {
    [H264Encoder::V4l2, H264Encoder::X264]
        .into_iter()
        .map(|encoder| PipelineCandidate {
            label: encoder.label(),
            description: build_h264_rtsp_pipeline_with_encoder(
                config_source,
                raw_caps,
                config,
                encoder,
            ),
        })
        .collect()
}

pub(crate) fn build_rpi_h264_rtsp_pipeline_candidates(
    config_source: &str,
    raw_caps: &str,
    config: &StreamConfig,
) -> Vec<PipelineCandidate> {
    vec![
        PipelineCandidate {
            label: H264Encoder::V4l2.label(),
            description: build_rpi_v4l2_rtsp_pipeline(config_source, raw_caps, config),
        },
        PipelineCandidate {
            label: H264Encoder::X264.label(),
            description: build_x264_rtsp_pipeline(config_source, raw_caps, config),
        },
    ]
}

fn build_h264_rtsp_pipeline_with_encoder(
    config_source: &str,
    raw_caps: &str,
    config: &StreamConfig,
    encoder: H264Encoder,
) -> String {
    match encoder {
        H264Encoder::V4l2 => build_v4l2_rtsp_pipeline(config_source, raw_caps, config),
        H264Encoder::X264 => build_x264_rtsp_pipeline(config_source, raw_caps, config),
    }
}

fn build_v4l2_rtsp_pipeline(config_source: &str, raw_caps: &str, config: &StreamConfig) -> String {
    format!(
        concat!(
            "{source} ",
            "! {raw_caps} ",
            "! queue leaky=downstream max-size-buffers=4 ",
            "! v4l2h264enc extra-controls=\"controls,video_bitrate={bitrate},repeat_sequence_header=1;\" ",
            "! video/x-h264,profile=(string)baseline ",
            "! h264parse config-interval=1 ",
            "! rtph264pay name=pay0 pt=96 config-interval=1 aggregate-mode=zero-latency mtu=1200"
        ),
        source = config_source,
        raw_caps = raw_caps,
        bitrate = config.bitrate(),
    )
}

fn build_rpi_v4l2_rtsp_pipeline(
    config_source: &str,
    raw_caps: &str,
    config: &StreamConfig,
) -> String {
    format!(
        concat!(
            "{source} ",
            "! capsfilter caps={raw_caps} ",
            "! v4l2h264enc extra-controls=\"controls,repeat_sequence_header=1,video_bitrate={bitrate}\" ",
            "! video/x-h264,profile=(string)baseline,level=(string)4 ",
            "! h264parse config-interval=1 ",
            "! queue leaky=downstream max-size-buffers=2 max-size-bytes=0 max-size-time=0 ",
            "! rtph264pay name=pay0 pt=96 config-interval=1 aggregate-mode=zero-latency mtu=1200"
        ),
        source = config_source,
        raw_caps = raw_caps,
        bitrate = config.bitrate(),
    )
}

fn build_x264_rtsp_pipeline(config_source: &str, raw_caps: &str, config: &StreamConfig) -> String {
    format!(
        concat!(
            "{source} ",
            "! {raw_caps} ",
            "! queue leaky=downstream max-size-buffers=4 ",
            "! videoconvert ",
            "! video/x-raw,format=I420 ",
            "! x264enc tune=zerolatency speed-preset=ultrafast bitrate={bitrate_kbps} key-int-max={framerate} bframes=0 byte-stream=true aud=true ",
            "! video/x-h264,profile=(string)baseline ",
            "! h264parse config-interval=1 ",
            "! rtph264pay name=pay0 pt=96 config-interval=1 aggregate-mode=zero-latency mtu=1200"
        ),
        source = config_source,
        raw_caps = raw_caps,
        bitrate_kbps = (config.bitrate() / 1000).max(1),
        framerate = config.framerate(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum H264Encoder {
    V4l2,
    X264,
}

impl H264Encoder {
    fn label(self) -> &'static str {
        match self {
            Self::V4l2 => "v4l2h264enc",
            Self::X264 => "x264enc",
        }
    }
}

fn build_vp8_rtsp_pipeline(config_source: &str, raw_caps: &str, config: &StreamConfig) -> String {
    format!(
        concat!(
            "{source} ",
            "! videoconvert ",
            "! {raw_caps} ",
            "! queue leaky=downstream max-size-buffers=4 ",
            "! vp8enc target-bitrate={bitrate} deadline=1 cpu-used=8 keyframe-max-dist={framerate} ",
            "! rtpvp8pay name=pay0 pt=96 mtu=1200 picture-id-mode=15-bit"
        ),
        source = config_source,
        raw_caps = raw_caps,
        framerate = config.framerate(),
        bitrate = config.bitrate(),
    )
}

#[cfg(test)]
mod tests {
    use crate::{Imx500Source, StreamConfig, StreamSource, pipeline};

    #[test]
    fn h264_pipeline_candidates_include_hardware_and_software_encoders() {
        let config = StreamConfig::new("127.0.0.1", 5000)
            .with_source(StreamSource::Imx500(Imx500Source::new()));

        let candidates = pipeline::build_stream_pipeline_candidates(&config);

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].label, "v4l2h264enc");
        assert!(candidates[0].description.contains("v4l2h264enc"));
        assert!(
            candidates[0]
                .description
                .contains("video/x-h264,profile=(string)baseline")
        );
        assert!(
            candidates[0]
                .description
                .contains("h264parse config-interval=1")
        );
        assert_eq!(candidates[1].label, "x264enc");
        assert!(candidates[1].description.contains("x264enc"));
        assert!(candidates[1].description.contains("videoconvert"));
        assert!(
            candidates[1]
                .description
                .contains("video/x-h264,profile=(string)baseline")
        );
        assert!(
            candidates[1]
                .description
                .contains("h264parse config-interval=1")
        );
    }
}
