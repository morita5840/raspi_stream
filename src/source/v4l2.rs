use crate::V4l2Source;
use crate::gst_support::build_v4l2src_fragment;

pub(crate) fn build_source_fragment(source: &V4l2Source) -> String {
    build_v4l2src_fragment(source.device_path(), None)
}

#[cfg(test)]
mod tests {
    use crate::{V4l2Source, source::v4l2};

    #[test]
    fn build_source_fragment_without_device_path() {
        assert_eq!(
            v4l2::build_source_fragment(&V4l2Source::new()),
            "v4l2src do-timestamp=true"
        );
    }

    #[test]
    fn build_source_fragment_escapes_device_path() {
        let source =
            v4l2::build_source_fragment(&V4l2Source::new().with_device_path("/dev/video\"2"));

        assert_eq!(
            source,
            "v4l2src do-timestamp=true device=\"/dev/video\\\"2\""
        );
    }
}
