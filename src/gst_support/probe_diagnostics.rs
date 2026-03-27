#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeSourceKind {
    Imx500,
    Libcamera,
    V4l2,
    VideoTest,
}

pub fn format_probe_resolution(
    source_kind: ProbeSourceKind,
    error_message: &str,
    v4l2_device_path: Option<&str>,
) -> String {
    let v4l2_device_path = v4l2_device_path.unwrap_or("/dev/video0");

    if matches!(
        source_kind,
        ProbeSourceKind::Imx500 | ProbeSourceKind::Libcamera
    ) && error_message.contains("no element \"libcamerasrc\"")
    {
        return String::from(
            ". install the GStreamer libcamera plugin, for example `sudo apt install gstreamer1.0-libcamera`, then verify with `gst-inspect-1.0 libcamerasrc`",
        );
    }

    if matches!(
        source_kind,
        ProbeSourceKind::Imx500 | ProbeSourceKind::Libcamera
    ) && error_message.contains("Could not find a camera named")
    {
        return String::from(
            ". omit `--camera-name` to use the default camera, or pass the exact libcamera camera id from `rpicam-hello --list-cameras`",
        );
    }

    if matches!(
        source_kind,
        ProbeSourceKind::Imx500 | ProbeSourceKind::Libcamera
    ) && (error_message.contains("no cameras available")
        || error_message.contains("No cameras available")
        || error_message.contains("Could not find any supported camera on this system")
        || error_message.contains("CameraManager::cameras() is empty")
        || error_message.contains("CameraMananger::cameras() is empty")
        || error_message.contains("camera manager"))
    {
        return String::from(
            ". libcamera did not detect any usable camera. verify the camera is connected and detected with `rpicam-hello --list-cameras`; if nothing is listed, check the CSI connection, camera firmware support, and that no other process is holding the camera",
        );
    }

    if matches!(
        source_kind,
        ProbeSourceKind::Imx500 | ProbeSourceKind::Libcamera
    ) && error_message.contains("Permission denied")
    {
        return String::from(
            ". the process does not have permission to access the camera stack. run with a user that can access the device, and check group membership and camera access policy on the Raspberry Pi",
        );
    }

    if matches!(
        source_kind,
        ProbeSourceKind::Imx500 | ProbeSourceKind::Libcamera
    ) && ["v4l2h264enc", "h264parse", "rtph264pay"]
        .iter()
        .any(|element| error_message.contains(&format!("no element \"{element}\"")))
    {
        return String::from(
            ". verify the Raspberry Pi GStreamer H.264 elements with `gst-inspect-1.0 v4l2h264enc h264parse rtph264pay`",
        );
    }

    if matches!(
        source_kind,
        ProbeSourceKind::Imx500 | ProbeSourceKind::Libcamera | ProbeSourceKind::V4l2
    ) && error_message.contains("no element \"x264enc\"")
    {
        return String::from(
            ". install the software H.264 fallback with `sudo apt install gstreamer1.0-plugins-ugly`, then verify with `gst-inspect-1.0 x264enc`",
        );
    }

    if matches!(
        source_kind,
        ProbeSourceKind::Imx500 | ProbeSourceKind::Libcamera | ProbeSourceKind::V4l2
    ) && (error_message.contains("gst_v4l2_video_enc_handle_frame")
        || error_message.contains("Failed to process frame."))
    {
        return String::from(
            ". the hardware H.264 encoder failed while processing frames. if this Raspberry Pi image lacks a working v4l2 H.264 path, install `gstreamer1.0-plugins-ugly` so raspi_stream can fall back to `x264enc`",
        );
    }

    if matches!(source_kind, ProbeSourceKind::V4l2)
        && error_message.contains("no element \"v4l2src\"")
    {
        return String::from(
            ". install the GStreamer video4linux plugin, for example `sudo apt install gstreamer1.0-plugins-good`, then verify with `gst-inspect-1.0 v4l2src`",
        );
    }

    if matches!(source_kind, ProbeSourceKind::V4l2)
        && (error_message.contains("Could not open device")
            || error_message.contains("Cannot identify device")
            || error_message.contains("No such file or directory"))
    {
        return format!(
            ". no usable V4L2 device node was found. verify a USB camera is connected, check `--device-path` points to an existing node such as `{v4l2_device_path}`, and inspect available devices with `v4l2-ctl --list-devices`",
        );
    }

    if matches!(source_kind, ProbeSourceKind::V4l2)
        && (error_message.contains("Device or resource busy")
            || error_message.contains("resource busy"))
    {
        return format!(
            ". the V4L2 device is busy. stop other camera processes, then check ownership with `fuser {v4l2_device_path}` or `lsof {v4l2_device_path}`",
        );
    }

    if matches!(source_kind, ProbeSourceKind::V4l2)
        && (error_message.contains("Failed to allocate required memory")
            || error_message.contains("Buffer pool activation failed"))
    {
        return format!(
            ". the V4L2 driver could not allocate capture buffers. another process may still be using the camera, the device may not support the requested mode, or the driver may require a different I/O path; first check `fuser {v4l2_device_path}`, then inspect supported formats with `v4l2-ctl --device {v4l2_device_path} --list-formats-ext`",
        );
    }

    if matches!(source_kind, ProbeSourceKind::V4l2)
        && (error_message.contains("not-negotiated")
            || error_message.contains("reason not-negotiated"))
    {
        return format!(
            ". the V4L2 device rejected the requested caps. many USB cameras expose MJPEG or only a subset of raw modes; inspect supported modes with `v4l2-ctl --device {v4l2_device_path} --list-formats-ext`, then retry with a matching `--width --height --framerate` or the correct `--device-path`",
        );
    }

    if matches!(source_kind, ProbeSourceKind::V4l2) && error_message.contains("Permission denied") {
        return format!(
            ". the process does not have permission to open the V4L2 node. verify device ownership with `ls -l {v4l2_device_path}` and ensure the user can access the `video` group or equivalent device permissions",
        );
    }

    if matches!(source_kind, ProbeSourceKind::VideoTest)
        && (error_message.contains("no element \"videotestsrc\"")
            || error_message.contains("no element \"vp8enc\"")
            || error_message.contains("no element \"rtpvp8pay\""))
    {
        return String::from(
            ". install the required GStreamer plugins and verify them with `gst-inspect-1.0 videotestsrc vp8enc rtpvp8pay`",
        );
    }

    String::new()
}

pub fn format_probe_guidance(hint: Option<&str>, resolution: &str) -> String {
    if !resolution.is_empty() {
        return resolution.to_string();
    }

    match hint {
        Some(hint) => format!(". {hint}"),
        None => String::new(),
    }
}

pub fn build_probe_search_text(error_message: &str, debug_text: Option<&str>) -> String {
    match debug_text {
        Some(debug_text) if !debug_text.is_empty() => {
            format!("{error_message} {debug_text}")
        }
        _ => error_message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProbeSourceKind, build_probe_search_text, format_probe_guidance, format_probe_resolution,
    };

    #[test]
    fn imx500_missing_libcamera_element_adds_install_hint() {
        let resolution =
            format_probe_resolution(ProbeSourceKind::Imx500, "no element \"libcamerasrc\"", None);

        assert!(resolution.contains("gstreamer1.0-libcamera"));
    }

    #[test]
    fn libcamera_camera_name_failure_adds_lookup_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::Libcamera,
            "Could not find a camera named",
            None,
        );

        assert!(resolution.contains("rpicam-hello --list-cameras"));
    }

    #[test]
    fn libcamera_no_cameras_available_adds_connection_hint() {
        let resolution =
            format_probe_resolution(ProbeSourceKind::Libcamera, "no cameras available", None);

        assert!(resolution.contains("rpicam-hello --list-cameras"));
        assert!(resolution.contains("CSI connection"));
    }

    #[test]
    fn libcamera_supported_camera_not_found_adds_connection_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::Libcamera,
            "Could not find any supported camera on this system",
            None,
        );

        assert!(resolution.contains("did not detect any usable camera"));
        assert!(resolution.contains("rpicam-hello --list-cameras"));
    }

    #[test]
    fn libcamera_empty_camera_list_adds_connection_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::Libcamera,
            "libcamera::CameraMananger::cameras() is empty",
            None,
        );

        assert!(resolution.contains("did not detect any usable camera"));
        assert!(resolution.contains("CSI connection"));
    }

    #[test]
    fn libcamera_permission_denied_adds_access_hint() {
        let resolution =
            format_probe_resolution(ProbeSourceKind::Libcamera, "Permission denied", None);

        assert!(resolution.contains("permission to access the camera stack"));
    }

    #[test]
    fn v4l2_missing_element_adds_install_hint() {
        let resolution =
            format_probe_resolution(ProbeSourceKind::V4l2, "no element \"v4l2src\"", None);

        assert!(resolution.contains("gstreamer1.0-plugins-good"));
    }

    #[test]
    fn v4l2_busy_device_adds_process_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::V4l2,
            "Device or resource busy",
            Some("/dev/video2"),
        );

        assert!(resolution.contains("fuser /dev/video2"));
    }

    #[test]
    fn v4l2_missing_device_adds_usb_camera_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::V4l2,
            "Cannot identify device '/dev/video0': No such file or directory",
            Some("/dev/video2"),
        );

        assert!(resolution.contains("USB camera is connected"));
        assert!(resolution.contains("v4l2-ctl --list-devices"));
    }

    #[test]
    fn v4l2_buffer_allocation_failure_adds_driver_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::V4l2,
            "Failed to allocate required memory. Buffer pool activation failed",
            Some("/dev/video2"),
        );

        assert!(resolution.contains("could not allocate capture buffers"));
        assert!(resolution.contains("fuser /dev/video2"));
        assert!(resolution.contains("v4l2-ctl --device /dev/video2 --list-formats-ext"));
    }

    #[test]
    fn build_probe_search_text_includes_debug_text() {
        let text = build_probe_search_text(
            "Internal data stream error.",
            Some("streaming stopped, reason not-negotiated (-4)"),
        );

        assert!(text.contains("Internal data stream error."));
        assert!(text.contains("reason not-negotiated (-4)"));
    }

    #[test]
    fn v4l2_permission_denied_adds_group_hint() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::V4l2,
            "Permission denied",
            Some("/dev/video2"),
        );

        assert!(resolution.contains("ls -l /dev/video2"));
        assert!(resolution.contains("video` group") || resolution.contains("video group"));
    }

    #[test]
    fn videotest_missing_plugin_adds_install_hint() {
        let resolution =
            format_probe_resolution(ProbeSourceKind::VideoTest, "no element \"vp8enc\"", None);

        assert!(resolution.contains("gst-inspect-1.0 videotestsrc vp8enc rtpvp8pay"));
    }

    #[test]
    fn v4l2_not_negotiated_uses_actual_device_path_when_available() {
        let resolution = format_probe_resolution(
            ProbeSourceKind::V4l2,
            "streaming stopped, reason not-negotiated (-4)",
            Some("/dev/video2"),
        );

        assert!(resolution.contains("v4l2-ctl --device /dev/video2 --list-formats-ext"));
    }

    #[test]
    fn specific_resolution_overrides_generic_hint() {
        let guidance = format_probe_guidance(Some("generic hint"), ". specific resolution hint");

        assert_eq!(guidance, ". specific resolution hint");
    }

    #[test]
    fn generic_hint_is_used_when_specific_resolution_is_empty() {
        let guidance = format_probe_guidance(Some("generic hint"), "");

        assert_eq!(guidance, ". generic hint");
    }
}
