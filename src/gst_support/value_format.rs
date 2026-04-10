pub(crate) fn escape_gst_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn sanitize_for_display(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Possible ANSI CSI sequence: skip until final byte in range '@'..='~'
            if let Some(&'[') = chars.peek() {
                chars.next();
                while let Some(&next_c) = chars.peek() {
                    chars.next();
                    if ('@'..='~').contains(&next_c) {
                        break;
                    }
                }
                continue;
            } else {
                continue;
            }
        }

        if c == '\n' || c == '\r' {
            out.push(' ');
        } else if c.is_control() {
            // skip other control chars
        } else {
            out.push(c);
        }
    }

    out.trim().to_string()
}

pub fn sanitize_videotest_pattern(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect()
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
    use super::{
        append_quoted_property, append_raw_property, escape_gst_value, sanitize_for_display,
        sanitize_videotest_pattern,
    };

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

    #[test]
    fn sanitize_videotest_pattern_strips_illegal_chars() {
        assert_eq!(
            sanitize_videotest_pattern("ball ! udpsink host=1"),
            "balludpsinkhost1"
        );
    }

    #[test]
    fn sanitize_for_display_strips_control_and_ansi_sequences() {
        assert_eq!(sanitize_for_display("hello\x1b[31mred\x1b[0m"), "hellored");
        assert_eq!(sanitize_for_display("line1\nline2"), "line1 line2");
    }
}
