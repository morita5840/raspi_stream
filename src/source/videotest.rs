use crate::VideoTestSource;
use crate::gst_support::build_videotestsrc_fragment;

pub(crate) fn build_source_fragment(source: &VideoTestSource) -> String {
    build_videotestsrc_fragment(source.is_live(), source.pattern(), None)
}

#[cfg(test)]
mod tests {
    use crate::{VideoTestSource, source::videotest};

    #[test]
    fn build_source_fragment_uses_defaults() {
        let source = videotest::build_source_fragment(&VideoTestSource::new());

        assert_eq!(source, "videotestsrc is-live=true");
    }

    #[test]
    fn build_source_fragment_uses_pattern() {
        let source = videotest::build_source_fragment(&VideoTestSource::new().with_pattern("ball"));

        assert_eq!(source, "videotestsrc is-live=true pattern=ball");
    }
}
