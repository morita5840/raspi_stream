use crate::{StreamConfig, pipeline::PipelineCandidate, source};

pub(crate) fn build_pipeline_candidates(config: &StreamConfig) -> Vec<PipelineCandidate> {
    let source = source::build_source_fragment(config);
    let raw_caps = format!(
        "video/x-raw,width={},height={},framerate={}/1",
        config.width(),
        config.height(),
        config.framerate(),
    );

    let raw_direct = source.clone();
    let raw_convert = format!(
        "{source} ! queue leaky=downstream max-size-buffers=4 ! videorate ! videoscale ! videoconvert"
    );
    let mjpeg_decode =
        format!("{source} ! image/jpeg ! jpegdec ! videorate ! videoscale ! videoconvert");

    let mut candidates = Vec::new();
    candidates.extend(build_variant_candidates(
        &raw_direct,
        &raw_caps,
        config,
        "v4l2h264enc/raw-direct",
        "x264enc/raw-direct",
    ));
    candidates.extend(build_variant_candidates(
        &raw_convert,
        &raw_caps,
        config,
        "v4l2h264enc/raw-convert",
        "x264enc/raw-convert",
    ));
    candidates.extend(build_variant_candidates(
        &mjpeg_decode,
        &raw_caps,
        config,
        "v4l2h264enc/mjpeg",
        "x264enc/mjpeg",
    ));

    candidates
}

fn build_variant_candidates(
    source: &str,
    raw_caps: &str,
    config: &StreamConfig,
    v4l2_label: &'static str,
    x264_label: &'static str,
) -> Vec<PipelineCandidate> {
    let candidates = super::build_h264_rtsp_pipeline_candidates(source, raw_caps, config);

    vec![
        PipelineCandidate {
            label: v4l2_label,
            description: candidates[0].description.clone(),
        },
        PipelineCandidate {
            label: x264_label,
            description: candidates[1].description.clone(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use crate::{StreamConfig, StreamSource, V4l2Source, pipeline};

    #[test]
    fn build_pipeline_uses_v4l2_source_fragment() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::V4l2(
            V4l2Source::new().with_device_path("/dev/video2"),
        ));

        let description = pipeline::build_stream_pipeline(&config);

        assert!(description.contains("v4l2src do-timestamp=true device=\"/dev/video2\""));
        assert!(!description.contains("format=NV12"));
        assert!(description.contains("v4l2h264enc"));
        assert!(description.contains("video/x-h264,profile=(string)baseline"));
        assert!(description.contains("rtph264pay name=pay0"));
        assert!(!description.contains("libcamerasrc"));
    }

    #[test]
    fn build_pipeline_candidates_include_raw_convert_and_mjpeg_variants() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::V4l2(
            V4l2Source::new().with_device_path("/dev/video2"),
        ));

        let candidates = pipeline::build_stream_pipeline_candidates(&config);

        assert_eq!(candidates.len(), 6);
        assert_eq!(candidates[2].label, "v4l2h264enc/raw-convert");
        assert!(
            candidates[2]
                .description
                .contains("videorate ! videoscale ! videoconvert")
        );
        assert_eq!(candidates[4].label, "v4l2h264enc/mjpeg");
        assert!(candidates[4].description.contains("image/jpeg ! jpegdec"));
        assert!(candidates[5].description.contains("x264enc"));
    }
}
