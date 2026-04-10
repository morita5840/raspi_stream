mod libcamera;
mod v4l2;
mod videotest;

use crate::{StreamConfig, StreamSource};

pub(crate) fn build_source_fragment(config: &StreamConfig) -> String {
    match config.source() {
        StreamSource::Imx500(source) => libcamera::build_imx500_source_fragment(source),
        StreamSource::Libcamera(source) => libcamera::build_source_fragment(source),
        StreamSource::V4l2(source) => v4l2::build_source_fragment(source),
        StreamSource::VideoTest(source) => videotest::build_source_fragment(source),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Imx500Source, LibcameraSource, StreamConfig, StreamSource, V4l2Source, VideoTestSource,
        source,
    };

    #[test]
    fn build_source_fragment_for_imx500_uses_default_camera() {
        let config = StreamConfig::new("127.0.0.1", 5000)
            .with_source(StreamSource::Imx500(Imx500Source::new()));

        let source = source::build_source_fragment(&config);

        assert_eq!(source, "libcamerasrc");
    }

    #[test]
    fn build_source_fragment_for_libcamera_uses_camera_name() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::Libcamera(
            LibcameraSource::new().with_camera_name("imx500"),
        ));

        let source = source::build_source_fragment(&config);

        assert_eq!(source, "libcamerasrc camera-name=\"imx500\"");
    }

    #[test]
    fn build_source_fragment_for_videotestsrc_returns_raw_element() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::VideoTest(
            VideoTestSource::new().with_pattern("ball"),
        ));

        let source = source::build_source_fragment(&config);

        assert_eq!(source, "videotestsrc is-live=true pattern=\"ball\"");
    }

    #[test]
    fn build_source_fragment_for_v4l2_uses_device_path() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::V4l2(
            V4l2Source::new().with_device_path("/dev/video2"),
        ));

        let source = source::build_source_fragment(&config);

        assert_eq!(source, "v4l2src do-timestamp=true device=\"/dev/video2\"");
    }
}
