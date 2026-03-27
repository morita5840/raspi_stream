use crate::gst_support::build_libcamerasrc_fragment;
use crate::{Imx500Source, LibcameraSource};

pub(crate) fn build_source_fragment(source: &LibcameraSource) -> String {
    build_fragment_from_camera_name(source.camera_name())
}

pub(crate) fn build_imx500_source_fragment(source: &Imx500Source) -> String {
    let mut fragment = build_fragment_from_camera_name(source.camera_name());
    let tuning = source.tuning();

    if let Some(exposure_time_us) = tuning.exposure_time_us() {
        append_uint_property(&mut fragment, "exposure-time", exposure_time_us);
    }

    if let Some(analogue_gain) = tuning.analogue_gain() {
        append_float_property(&mut fragment, "analogue-gain", analogue_gain);
    }

    if let Some(brightness) = tuning.brightness() {
        append_float_property(&mut fragment, "brightness", brightness);
    }

    if let Some(contrast) = tuning.contrast() {
        append_float_property(&mut fragment, "contrast", contrast);
    }

    if let Some(saturation) = tuning.saturation() {
        append_float_property(&mut fragment, "saturation", saturation);
    }

    if let Some(sharpness) = tuning.sharpness() {
        append_float_property(&mut fragment, "sharpness", sharpness);
    }

    fragment
}

fn build_fragment_from_camera_name(camera_name: Option<&str>) -> String {
    build_libcamerasrc_fragment(camera_name, None)
}

fn append_uint_property(fragment: &mut String, property_name: &str, value: u32) {
    fragment.push(' ');
    fragment.push_str(property_name);
    fragment.push('=');
    fragment.push_str(&value.to_string());
}

fn append_float_property(fragment: &mut String, property_name: &str, value: f32) {
    fragment.push(' ');
    fragment.push_str(property_name);
    fragment.push('=');
    fragment.push_str(&format_float(value));
}

fn format_float(value: f32) -> String {
    let mut formatted = value.to_string();
    if !formatted.contains('.') && !formatted.contains('e') && !formatted.contains('E') {
        formatted.push_str(".0");
    }
    formatted
}

#[cfg(test)]
mod tests {
    use crate::{Imx500Source, Imx500Tuning, LibcameraSource, source::libcamera};

    #[test]
    fn build_source_fragment_without_camera_name() {
        assert_eq!(
            libcamera::build_source_fragment(&LibcameraSource::new()),
            "libcamerasrc"
        );
    }

    #[test]
    fn build_source_fragment_escapes_camera_name() {
        let source = libcamera::build_source_fragment(
            &LibcameraSource::new().with_camera_name("imx500\"main"),
        );

        assert_eq!(source, "libcamerasrc camera-name=\"imx500\\\"main\"");
    }

    #[test]
    fn build_imx500_source_fragment_without_camera_name_uses_default_camera() {
        let source = libcamera::build_imx500_source_fragment(&Imx500Source::new());

        assert_eq!(source, "libcamerasrc");
    }

    #[test]
    fn build_imx500_source_fragment_uses_explicit_camera_name() {
        let source = libcamera::build_imx500_source_fragment(
            &Imx500Source::new().with_camera_name("ai-main"),
        );

        assert_eq!(source, "libcamerasrc camera-name=\"ai-main\"");
    }

    #[test]
    fn build_imx500_source_fragment_includes_typed_imx500_controls() {
        let source = libcamera::build_imx500_source_fragment(
            &Imx500Source::new().with_tuning(
                Imx500Tuning::new()
                    .with_exposure_time_us(10_000)
                    .with_analogue_gain(2.5)
                    .with_brightness(0.1)
                    .with_contrast(1.2)
                    .with_saturation(1.3)
                    .with_sharpness(0.8),
            ),
        );

        assert_eq!(
            source,
            concat!(
                "libcamerasrc ",
                "exposure-time=10000 ",
                "analogue-gain=2.5 ",
                "brightness=0.1 ",
                "contrast=1.2 ",
                "saturation=1.3 ",
                "sharpness=0.8"
            )
        );
    }
}
