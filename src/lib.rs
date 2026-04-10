//! Raspberry Pi 向け映像配信ライブラリ.
//!
//! 現在は公開 API の最小単位として [`StreamConfig`], [`CameraStreamer`], [`StreamSession`], [`StreamEvent`] を提供する.
//! `StreamConfig` は配信先や解像度など, 配信開始前に必要な設定を保持する.
//! `CameraStreamer` はその設定を保持する配信オブジェクト.
//! `StreamSession` は配信開始 API が返す, 実行中セッションの型.
//! `StreamEvent` は実行中通知を表す型.
//!
//! # Examples
//!
//! ```no_run
//! use std::time::Duration;
//!
//! use raspi_stream::{CameraStreamer, Imx500Source, Imx500Tuning, StreamConfig, StreamSource};
//!
//! let config = StreamConfig::new("0.0.0.0", 8554)
//!     .with_resolution(1280, 720)
//!     .with_framerate(20)
//!     .with_bitrate(2_000_000)
//!     .with_stream_path("/camera")
//!     .with_source(StreamSource::Imx500(
//!         Imx500Source::new()
//!             .with_tuning(
//!                 Imx500Tuning::new()
//!                     .with_exposure_time_us(10_000)
//!                     .with_analogue_gain(2.0),
//!             ),
//!     ));
//!
//! let streamer = CameraStreamer::new(config.clone());
//! assert_eq!(streamer.config(), &config);
//! let session = streamer.start()?;
//! assert_eq!(
//!     session.poll_event(Duration::from_millis(100)),
//!     Some(raspi_stream::StreamEvent::Started {
//!         stream_url: "rtsp://0.0.0.0:8554/camera".to_string(),
//!     })
//! );
//! session.stop()?;
//! # Ok::<(), raspi_stream::StreamError>(())
//! ```

mod config;
mod diagnostic;
mod error;
mod event;
#[doc(hidden)]
pub mod gst_support;
mod pipeline;
mod runtime;
mod source;
mod streamer;

pub use config::{
    Imx500Source, Imx500Tuning, LibcameraSource, StreamConfig, StreamSource, V4l2Source,
    VideoTestSource,
};
pub use diagnostic::{StartupDiagnostic, StartupDiagnosticKind};
pub use error::StreamError;
pub use event::StreamEvent;
pub use streamer::{CameraStreamer, StreamSession};

#[cfg(test)]
pub(crate) fn ensure_gstreamer_init_for_tests() {
    static GST_INIT: std::sync::Once = std::sync::Once::new();

    GST_INIT.call_once(|| {
        gstreamer::init().expect("gstreamer init should succeed");
    });
}
