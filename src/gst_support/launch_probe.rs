use gst::prelude::*;
use gstreamer as gst;

pub const DEFAULT_PROBE_TIMEOUT_MS: u64 = 700;

#[derive(Debug)]
pub enum ProbeBuildError {
    #[allow(dead_code)]
    Parse(gst::glib::Error),
    NotPipeline,
    MissingBus,
}

pub struct LaunchPipelineProbe {
    pipeline: gst::Pipeline,
    bus: gst::Bus,
}

impl LaunchPipelineProbe {
    pub fn from_launch(description: &str) -> Result<Self, ProbeBuildError> {
        let element = gst::parse::launch(description).map_err(ProbeBuildError::Parse)?;
        let pipeline = element
            .dynamic_cast::<gst::Pipeline>()
            .map_err(|_| ProbeBuildError::NotPipeline)?;
        let bus = pipeline.bus().ok_or(ProbeBuildError::MissingBus)?;

        Ok(Self { pipeline, bus })
    }

    pub fn start(&self) -> Result<(), gst::StateChangeError> {
        self.pipeline.set_state(gst::State::Playing).map(|_| ())
    }

    #[allow(dead_code)]
    pub fn bus(&self) -> &gst::Bus {
        &self.bus
    }

    pub fn timed_pop_filtered(
        &self,
        timeout_ms: u64,
        message_types: &[gst::MessageType],
    ) -> Option<gst::Message> {
        self.bus
            .timed_pop_filtered(gst::ClockTime::from_mseconds(timeout_ms), message_types)
    }

    pub fn shutdown(&self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_PROBE_TIMEOUT_MS, LaunchPipelineProbe, ProbeBuildError};
    use gstreamer as gst;

    #[test]
    fn from_launch_returns_parse_error_for_invalid_description() {
        gst::init().expect("gstreamer init should succeed");

        let result = LaunchPipelineProbe::from_launch("!");

        assert!(matches!(result, Err(ProbeBuildError::Parse(_))));
    }

    #[test]
    fn from_launch_rejects_non_pipeline_element() {
        gst::init().expect("gstreamer init should succeed");

        let result = LaunchPipelineProbe::from_launch("fakesink");

        assert!(matches!(result, Err(ProbeBuildError::NotPipeline)));
    }

    #[test]
    fn from_launch_can_start_and_receive_non_error_message() {
        gst::init().expect("gstreamer init should succeed");

        if gst::ElementFactory::find("fakesrc").is_none()
            || gst::ElementFactory::find("fakesink").is_none()
        {
            return;
        }

        let probe = LaunchPipelineProbe::from_launch(
            "fakesrc num-buffers=1 ! fakesink sync=false async=false",
        )
        .expect("probe pipeline should be created");

        assert!(probe.start().is_ok());

        let message = probe.timed_pop_filtered(
            DEFAULT_PROBE_TIMEOUT_MS,
            &[
                gst::MessageType::Error,
                gst::MessageType::AsyncDone,
                gst::MessageType::Eos,
            ],
        );

        assert!(!matches!(
            message.as_ref().map(|message| message.view()),
            Some(gst::MessageView::Error(_))
        ));

        probe.shutdown();
    }
}
