pub(crate) fn escape_gst_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn append_quoted_property(fragment: &mut String, property_name: &str, value: &str) {
    fragment.push(' ');
    fragment.push_str(property_name);
    fragment.push_str("=\"");
    fragment.push_str(&escape_gst_value(value));
    fragment.push('"');
}

pub(crate) fn append_raw_property(
    fragment: &mut String,
    property_name: &str,
    value: impl std::fmt::Display,
) {
    fragment.push(' ');
    fragment.push_str(property_name);
    fragment.push('=');
    fragment.push_str(&value.to_string());
}

#[cfg(test)]
mod tests {
    use super::{append_quoted_property, append_raw_property, escape_gst_value};

    #[test]
    fn escape_gst_value_escapes_backslashes_and_quotes() {
        assert_eq!(escape_gst_value("a\\b\"c"), "a\\\\b\\\"c");
    }

    #[test]
    fn append_quoted_property_formats_assignment() {
        let mut fragment = String::from("libcamerasrc");

        append_quoted_property(&mut fragment, "camera-name", "imx500\"main");

        assert_eq!(fragment, "libcamerasrc camera-name=\"imx500\\\"main\"");
    }

    #[test]
    fn append_raw_property_formats_assignment() {
        let mut fragment = String::from("videotestsrc");

        append_raw_property(&mut fragment, "is-live", true);

        assert_eq!(fragment, "videotestsrc is-live=true");
    }
}
