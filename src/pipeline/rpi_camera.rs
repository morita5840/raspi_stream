use crate::{StreamConfig, StreamSource, pipeline::PipelineCandidate, source};

pub(crate) fn build_pipeline_candidates(config: &StreamConfig) -> Vec<PipelineCandidate> {
    let source = source::build_source_fragment(config);

    debug_assert!(matches!(
        config.source(),
        StreamSource::Imx500(..) | StreamSource::Libcamera(..)
    ));

    build_libcamera_rtsp_pipeline_candidates(config, &source)
}

fn build_libcamera_rtsp_pipeline_candidates(
    config: &StreamConfig,
    source: &str,
) -> Vec<PipelineCandidate> {
    super::build_rpi_h264_rtsp_pipeline_candidates(
        source,
        &format!(
            "video/x-raw,width={},height={},format=NV12,framerate={}/1",
            config.width(),
            config.height(),
            config.framerate(),
        ),
        config,
    )
}

#[cfg(test)]
mod tests {
    use crate::{Imx500Source, LibcameraSource, StreamConfig, StreamSource, pipeline};

    #[test]
    fn build_pipeline_uses_imx500_rtsp_pipeline_for_raspberry_pi_ai_camera() {
        let config = StreamConfig::new("127.0.0.1", 5000)
            .with_source(StreamSource::Imx500(Imx500Source::new()));

        let description = pipeline::build_stream_pipeline(&config);

        assert!(description.contains("libcamerasrc ! capsfilter caps=video/x-raw"));
        assert!(!description.contains("video/x-raw,width=127.0"));
        assert!(
            description.contains("video/x-raw,width=1280,height=720,format=NV12,framerate=20/1")
        );
        assert!(description.contains("v4l2h264enc"));
        assert!(description.contains("video/x-h264,profile=(string)baseline,level=(string)4"));
        assert!(description.contains("h264parse config-interval=1"));
        assert!(description.contains(
            "queue leaky=downstream max-size-buffers=2 max-size-bytes=0 max-size-time=0"
        ));
        assert!(description.contains("rtph264pay name=pay0"));
    }

    #[test]
    fn build_pipeline_escapes_camera_name() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::Libcamera(
            LibcameraSource::new().with_camera_name("imx500\"main"),
        ));

        let description = pipeline::build_stream_pipeline(&config);

        assert!(description.contains("camera-name=\"imx500\\\"main\""));
    }

    #[test]
    fn build_pipeline_uses_libcamera_rtsp_pipeline_for_raspberry_pi() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::Libcamera(
            LibcameraSource::new().with_camera_name("imx500"),
        ));

        let description = pipeline::build_stream_pipeline(&config);

        assert!(description.contains("libcamerasrc camera-name=\"imx500\""));
        assert!(
            description.contains("video/x-raw,width=1280,height=720,format=NV12,framerate=20/1")
        );
        assert!(description.contains("v4l2h264enc"));
        assert!(description.contains("video/x-h264,profile=(string)baseline,level=(string)4"));
        assert!(description.contains("h264parse config-interval=1"));
        assert!(description.contains(
            "queue leaky=downstream max-size-buffers=2 max-size-bytes=0 max-size-time=0"
        ));
        assert!(description.contains("rtph264pay name=pay0"));
    }
}
