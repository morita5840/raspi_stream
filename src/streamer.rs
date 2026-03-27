use std::time::Duration;

use crate::{StartupDiagnostic, StreamConfig, StreamEvent, runtime::RuntimeSessionHandle};

/// 配信設定を保持する配信オブジェクト.
///
/// [`StreamConfig`] を保持し, RTSP 配信セッションを開始するための入口になる型.
///
/// # Examples
///
/// ```no_run
/// use raspi_stream::{CameraStreamer, Imx500Source, Imx500Tuning, StreamConfig, StreamSource};
///
/// let config = StreamConfig::new("0.0.0.0", 8554)
///     .with_stream_path("/camera")
///     .with_source(StreamSource::Imx500(
///     Imx500Source::new().with_tuning(Imx500Tuning::new().with_exposure_time_us(10_000)),
/// ));
/// let streamer = CameraStreamer::new(config.clone());
///
/// assert_eq!(streamer.config(), &config);
/// let session = streamer.start()?;
/// assert_eq!(
///     session.poll_event(std::time::Duration::from_millis(100)),
///     Some(raspi_stream::StreamEvent::Started {
///         stream_url: "rtsp://0.0.0.0:8554/camera".to_string(),
///     })
/// );
/// session.stop()?;
/// # Ok::<(), raspi_stream::StreamError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraStreamer {
    config: StreamConfig,
}

/// 実行中配信セッションを表す型.
///
/// [`CameraStreamer::start()`] が返す実行中の RTSP 配信セッション.
/// [`StreamSession::poll_event()`] で状態変化を取得し, [`StreamSession::stop()`] で停止する.
#[derive(Debug, Clone)]
pub struct StreamSession {
    runtime: RuntimeSessionHandle,
}

impl Default for StreamSession {
    fn default() -> Self {
        Self {
            runtime: RuntimeSessionHandle::inert(),
        }
    }
}

impl CameraStreamer {
    /// 配信設定から配信オブジェクトを生成する.
    pub fn new(config: StreamConfig) -> Self {
        Self { config }
    }

    /// 保持している配信設定を返す.
    pub fn config(&self) -> &StreamConfig {
        &self.config
    }

    /// 配信を開始する.
    ///
    /// 設定を検証し, RTSP server を起動した状態の [`StreamSession`] を返す.
    ///
    /// # Errors
    ///
    /// 以下の場合に [`crate::StreamError`] を返す.
    ///
    /// - [`StreamConfig::validate()`] に失敗した場合
    /// - GStreamer の初期化に失敗した場合
    /// - RTSP server の起動に失敗した場合
    pub fn start(&self) -> Result<StreamSession, crate::StreamError> {
        self.config.validate()?;
        Ok(StreamSession {
            runtime: RuntimeSessionHandle::start(&self.config)?,
        })
    }
}

impl StreamSession {
    /// 実行中イベントを取得する.
    ///
    /// 指定した待ち時間まで次の [`StreamEvent`] を待ち, 受信できれば返す.
    ///
    /// 現在の RTSP runtime では, 開始直後に [`StreamEvent::Started`], 実行中異常時に
    /// [`StreamEvent::Error`], 終了時に [`StreamEvent::Stopped`] を返す.
    pub fn poll_event(&self, timeout: Duration) -> Option<StreamEvent> {
        self.runtime.poll_event(timeout)
    }

    /// 実際に選ばれた pipeline 候補ラベルを返す.
    pub fn pipeline_label(&self) -> String {
        self.runtime.pipeline_label()
    }

    /// 起動時にスキップされた pipeline 候補の詳細診断文字列を返す.
    pub fn startup_diagnostics(&self) -> Vec<String> {
        self.runtime
            .startup_diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.verbose_line())
            .collect()
    }

    /// 起動時にスキップされた pipeline 候補の構造化診断情報を返す.
    pub fn startup_diagnostic_entries(&self) -> Vec<StartupDiagnostic> {
        self.runtime.startup_diagnostics()
    }

    /// 配信セッションを停止する.
    ///
    /// 停止処理は冪等で, すでに停止済みのセッションに対して呼んでも成功する.
    /// 明示停止時の [`StreamEvent::Stopped`] は最初の停止で 1 回だけ返る.
    ///
    /// # Errors
    ///
    /// RTSP server 側の停止処理で内部状態の同期に失敗した場合に
    /// [`crate::StreamError`] を返す.
    pub fn stop(&self) -> Result<(), crate::StreamError> {
        self.runtime.stop()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        CameraStreamer, Imx500Source, StreamConfig, StreamError, StreamEvent, StreamSession,
        StreamSource, V4l2Source, VideoTestSource, pipeline,
    };

    fn development_source() -> Option<StreamSource> {
        gstreamer::init().expect("gstreamer init should succeed");

        gstreamer::ElementFactory::find("videotestsrc")
            .map(|_| StreamSource::VideoTest(VideoTestSource::new().with_pattern("ball")))
    }

    #[test]
    fn new_stores_config() {
        let config = StreamConfig::new("192.168.1.10", 5000)
            .with_source(StreamSource::VideoTest(
                VideoTestSource::new().with_pattern("ball"),
            ))
            .with_resolution(640, 480)
            .with_framerate(15)
            .with_bitrate(1_000_000);

        let streamer = CameraStreamer::new(config.clone());

        assert_eq!(streamer.config(), &config);
    }

    #[test]
    fn inert_session_has_no_event() {
        let session = StreamSession::default();

        assert_eq!(session.poll_event(Duration::from_millis(1)), None);
    }

    #[test]
    fn start_returns_session_when_config_is_valid() {
        let Some(source) = development_source() else {
            return;
        };

        let streamer = CameraStreamer::new(
            StreamConfig::new("127.0.0.1", 18555)
                .with_stream_path("/test")
                .with_source(source),
        );

        assert!(streamer.start().is_ok());
    }

    #[test]
    fn start_returns_error_when_config_is_invalid() {
        let streamer = CameraStreamer::new(StreamConfig::new("", 5000));

        assert!(matches!(
            streamer.start(),
            Err(StreamError::InvalidConfig(message)) if message == "bind_host must not be empty"
        ));
    }

    #[test]
    fn stop_succeeds_for_default_session() {
        let session = StreamSession::default();

        assert_eq!(session.stop(), Ok(()));
    }

    #[test]
    fn poll_event_returns_none_when_no_event_source_exists() {
        let session = StreamSession::default();

        assert_eq!(session.poll_event(Duration::from_millis(10)), None);
    }

    #[test]
    fn start_builds_raspberry_pi_pipeline_description() {
        let config = StreamConfig::new("0.0.0.0", 8554)
            .with_stream_path("/camera")
            .with_source(StreamSource::Imx500(Imx500Source::new()))
            .with_resolution(640, 480)
            .with_framerate(15)
            .with_bitrate(1_000_000);

        let description = pipeline::build_stream_pipeline(&config);

        assert!(description.contains("libcamerasrc ! capsfilter caps=video/x-raw"));
        assert!(description.contains("width=640"));
        assert!(description.contains("height=480"));
        assert!(description.contains("format=NV12"));
        assert!(description.contains("framerate=15/1"));
        assert!(description.contains("video_bitrate=1000000"));
        assert!(description.contains("name=pay0"));
    }

    #[test]
    fn start_works_with_wsl_videotestsrc() {
        let Some(source) = development_source() else {
            return;
        };

        let streamer = CameraStreamer::new(
            StreamConfig::new("127.0.0.1", 18556)
                .with_stream_path("/test")
                .with_source(source),
        );

        let session = streamer.start().expect("videotestsrc start should succeed");

        assert!(matches!(
            session.poll_event(Duration::from_millis(100)),
            Some(StreamEvent::Started { .. })
        ));
        assert_eq!(session.stop(), Ok(()));
    }

    #[test]
    fn start_rejects_unusable_videotest_configuration() {
        let Some(_) = development_source() else {
            return;
        };

        let streamer = CameraStreamer::new(
            StreamConfig::new("127.0.0.1", 18559)
                .with_stream_path("/test")
                .with_source(StreamSource::VideoTest(
                    VideoTestSource::new().with_pattern("definitely-invalid-pattern"),
                )),
        );

        assert!(matches!(
            streamer.start(),
            Err(StreamError::PipelineBuildFailed(_)) | Err(StreamError::RuntimeError(_))
        ));
    }

    #[test]
    fn start_builds_v4l2_pipeline_description() {
        let config = StreamConfig::new("0.0.0.0", 8554)
            .with_stream_path("/camera")
            .with_source(StreamSource::V4l2(
                V4l2Source::new().with_device_path("/dev/video2"),
            ))
            .with_resolution(640, 480)
            .with_framerate(15)
            .with_bitrate(1_000_000);

        let description = pipeline::build_stream_pipeline(&config);

        assert!(description.contains("v4l2src do-timestamp=true device=\"/dev/video2\""));
        assert!(description.contains("width=640"));
        assert!(description.contains("height=480"));
        assert!(description.contains("framerate=15/1"));
        assert!(description.contains("video_bitrate=1000000"));
        assert!(description.contains("name=pay0"));
    }
}
