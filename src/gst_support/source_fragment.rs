use crate::gst_support::value_format::{append_quoted_property, append_raw_property};

pub fn build_libcamerasrc_fragment(camera_name: Option<&str>, num_buffers: Option<u32>) -> String {
    let mut fragment = String::from("libcamerasrc");

    if let Some(num_buffers) = num_buffers {
        append_raw_property(&mut fragment, "num-buffers", num_buffers);
    }

    if let Some(camera_name) = camera_name {
        append_quoted_property(&mut fragment, "camera-name", camera_name);
    }

    fragment
}

pub fn build_v4l2src_fragment(device_path: Option<&str>, num_buffers: Option<u32>) -> String {
    let mut fragment = String::from("v4l2src");
    append_raw_property(&mut fragment, "do-timestamp", true);

    if let Some(num_buffers) = num_buffers {
        append_raw_property(&mut fragment, "num-buffers", num_buffers);
    }

    if let Some(device_path) = device_path {
        append_quoted_property(&mut fragment, "device", device_path);
    }

    fragment
}

pub fn build_videotestsrc_fragment(
    is_live: bool,
    pattern: Option<&str>,
    num_buffers: Option<u32>,
) -> String {
    let mut fragment = String::from("videotestsrc");

    if let Some(num_buffers) = num_buffers {
        append_raw_property(&mut fragment, "num-buffers", num_buffers);
    }

    append_raw_property(&mut fragment, "is-live", is_live);

    if let Some(pattern) = pattern {
        append_raw_property(&mut fragment, "pattern", pattern);
    }

    fragment
}

#[cfg(test)]
mod tests {
    use super::{build_libcamerasrc_fragment, build_v4l2src_fragment, build_videotestsrc_fragment};

    #[test]
    fn libcamerasrc_fragment_supports_optional_num_buffers_and_camera_name() {
        assert_eq!(
            build_libcamerasrc_fragment(Some("imx500\"main"), Some(1)),
            "libcamerasrc num-buffers=1 camera-name=\"imx500\\\"main\""
        );
    }

    #[test]
    fn v4l2src_fragment_keeps_do_timestamp_and_optional_device() {
        assert_eq!(
            build_v4l2src_fragment(Some("/dev/video2"), Some(1)),
            "v4l2src do-timestamp=true num-buffers=1 device=\"/dev/video2\""
        );
    }

    #[test]
    fn videotestsrc_fragment_supports_optional_pattern() {
        assert_eq!(
            build_videotestsrc_fragment(true, Some("ball"), Some(1)),
            "videotestsrc num-buffers=1 is-live=true pattern=ball"
        );
    }
}
