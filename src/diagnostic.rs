#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupDiagnosticKind {
    CameraCapsRejected,
    HardwareEncoderFailed,
    BufferAllocationFailed,
    DeviceBusy,
    PermissionDenied,
    MissingElement,
    StartupProbeFailed,
}

impl StartupDiagnosticKind {
    pub fn short_reason(self) -> &'static str {
        match self {
            Self::CameraCapsRejected => "camera caps were rejected",
            Self::HardwareEncoderFailed => "hardware H.264 encoder failed",
            Self::BufferAllocationFailed => "V4L2 buffer allocation failed",
            Self::DeviceBusy => "device is busy",
            Self::PermissionDenied => "permission denied",
            Self::MissingElement => "required GStreamer element is missing",
            Self::StartupProbeFailed => "startup probe failed",
        }
    }

    fn from_message(message: &str) -> Self {
        if message.contains("not-negotiated") || message.contains("reason not-negotiated") {
            return Self::CameraCapsRejected;
        }

        if message.contains("Failed to process frame.")
            || message.contains("gst_v4l2_video_enc_handle_frame")
        {
            return Self::HardwareEncoderFailed;
        }

        if message.contains("Failed to allocate required memory")
            || message.contains("Buffer pool activation failed")
        {
            return Self::BufferAllocationFailed;
        }

        if message.contains("Device or resource busy") || message.contains("resource busy") {
            return Self::DeviceBusy;
        }

        if message.contains("Permission denied") {
            return Self::PermissionDenied;
        }

        if message.contains("no element") {
            return Self::MissingElement;
        }

        Self::StartupProbeFailed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupDiagnostic {
    candidate_label: String,
    detailed_message: String,
    kind: StartupDiagnosticKind,
}

impl StartupDiagnostic {
    pub fn new(candidate_label: impl Into<String>, detailed_message: impl Into<String>) -> Self {
        let detailed_message = detailed_message.into();

        Self {
            candidate_label: candidate_label.into(),
            kind: StartupDiagnosticKind::from_message(&detailed_message),
            detailed_message,
        }
    }

    pub fn candidate_label(&self) -> &str {
        &self.candidate_label
    }

    pub fn detailed_message(&self) -> &str {
        &self.detailed_message
    }

    pub fn kind(&self) -> StartupDiagnosticKind {
        self.kind
    }

    pub fn summary_line(&self) -> String {
        format!(
            "startup fallback: skipped {} ({})",
            self.candidate_label,
            self.kind.short_reason()
        )
    }

    pub fn verbose_line(&self) -> String {
        format!(
            "startup fallback: skipped {} because {}",
            self.candidate_label, self.detailed_message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{StartupDiagnostic, StartupDiagnosticKind};

    #[test]
    fn startup_diagnostic_detects_not_negotiated_as_caps_rejected() {
        let diagnostic = StartupDiagnostic::new(
            "x264enc/raw-direct",
            "startup probe failed for v4l2 device \"/dev/video2\": Internal data stream error. reason not-negotiated (-4)",
        );

        assert_eq!(diagnostic.kind(), StartupDiagnosticKind::CameraCapsRejected);
        assert_eq!(
            diagnostic.summary_line(),
            "startup fallback: skipped x264enc/raw-direct (camera caps were rejected)"
        );
    }

    #[test]
    fn startup_diagnostic_formats_verbose_line() {
        let diagnostic =
            StartupDiagnostic::new("v4l2h264enc/raw-convert", "Failed to process frame.");

        assert_eq!(
            diagnostic.verbose_line(),
            "startup fallback: skipped v4l2h264enc/raw-convert because Failed to process frame."
        );
    }
}
