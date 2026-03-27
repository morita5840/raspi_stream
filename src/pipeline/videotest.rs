use crate::{StreamConfig, source};

pub(crate) fn build_pipeline(config: &StreamConfig) -> String {
    let source = source::build_source_fragment(config);

    super::build_vp8_rtsp_pipeline(
        &source,
        &format!(
            "video/x-raw,format=I420,width={},height={},framerate={}/1",
            config.width(),
            config.height(),
            config.framerate(),
        ),
        config,
    )
}

#[cfg(test)]
mod tests {
    use crate::{StreamConfig, StreamSource, VideoTestSource, pipeline};

    #[test]
    fn build_pipeline_uses_videotestsrc_pipeline_for_wsl() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::VideoTest(
            VideoTestSource::new().with_pattern("ball"),
        ));

        let description = pipeline::build_stream_pipeline(&config);

        assert!(description.contains("videotestsrc is-live=true pattern=ball"));
        assert!(description.contains("videoconvert"));
        assert!(description.contains("video/x-raw,format=I420"));
        assert!(description.contains("vp8enc"));
        assert!(description.contains("rtpvp8pay name=pay0"));
        assert!(!description.contains("libcamerasrc"));
        assert!(!description.contains("v4l2h264enc"));
    }
}
