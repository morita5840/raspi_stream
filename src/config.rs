use crate::StreamError;

/// 配信入力ソースを表す型.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamSource {
    /// Raspberry Pi AI Camera IMX500 用のソース
    Imx500(Imx500Source),
    /// Raspberry Pi 標準カメラ用ソース
    Libcamera(LibcameraSource),
    /// USB カメラ等の V4L2 デバイス用ソース
    V4l2(V4l2Source),
    /// テスト用仮想ソース
    VideoTest(VideoTestSource),
}

/// Raspberry Pi AI Camera IMX500 用の設定.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imx500Source {
    /// 利用するカメラ名. 未指定時は既定カメラを使う想定.
    camera_name: Option<String>,
    /// IMX500 固有の tuning 設定.
    tuning: Imx500Tuning,
}

/// Raspberry Pi AI Camera IMX500 固有の tuning 設定.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imx500Tuning {
    /// 露光時間の上書き値. 単位は usec.
    exposure_time_us: Option<u32>,
    /// アナログゲインの上書き値.
    analogue_gain: Option<OrderedF32>,
    /// 明るさ補正の上書き値.
    brightness: Option<OrderedF32>,
    /// コントラスト補正の上書き値.
    contrast: Option<OrderedF32>,
    /// 彩度補正の上書き値.
    saturation: Option<OrderedF32>,
    /// シャープネス補正の上書き値.
    sharpness: Option<OrderedF32>,
}

/// `Eq` を保ったまま `f32` を保持するための薄い wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrderedF32(u32);

/// `libcamerasrc` 用の設定.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibcameraSource {
    /// 利用するカメラ名. 未指定時は既定カメラを使う想定.
    camera_name: Option<String>,
}

/// `v4l2src` 用の設定.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V4l2Source {
    /// 利用するデバイスパス. 未指定時は既定デバイスを使う想定.
    device_path: Option<String>,
}

/// `videotestsrc` 用の設定.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTestSource {
    /// live source として扱うか.
    is_live: bool,
    /// `videotestsrc` の pattern 名.
    pattern: Option<String>,
}

impl StreamSource {
    /// IMX500 向けソースを返す.
    pub fn imx500() -> Self {
        Self::Imx500(Imx500Source::new())
    }

    /// `libcamerasrc` を使うソースを返す.
    pub fn libcamera() -> Self {
        Self::Libcamera(LibcameraSource::new())
    }

    /// `videotestsrc` を使うソースを返す.
    pub fn videotest() -> Self {
        Self::VideoTest(VideoTestSource::new())
    }

    /// `v4l2src` を使うソースを返す.
    pub fn v4l2() -> Self {
        Self::V4l2(V4l2Source::new())
    }
}

impl Imx500Source {
    /// IMX500 用の既定設定を返す.
    pub fn new() -> Self {
        Self {
            camera_name: None,
            tuning: Imx500Tuning::new(),
        }
    }

    /// 利用するカメラ名を設定する.
    pub fn with_camera_name(mut self, camera_name: impl Into<String>) -> Self {
        self.camera_name = Some(camera_name.into());
        self
    }

    /// 利用するカメラ名を返す.
    pub fn camera_name(&self) -> Option<&str> {
        self.camera_name.as_deref()
    }

    /// IMX500 固有の tuning 設定をまとめて上書きする.
    pub fn with_tuning(mut self, tuning: Imx500Tuning) -> Self {
        self.tuning = tuning;
        self
    }

    /// IMX500 固有の tuning 設定を返す.
    pub fn tuning(&self) -> &Imx500Tuning {
        &self.tuning
    }
}

impl Imx500Tuning {
    /// IMX500 tuning の既定設定を返す.
    pub fn new() -> Self {
        Self {
            exposure_time_us: None,
            analogue_gain: None,
            brightness: None,
            contrast: None,
            saturation: None,
            sharpness: None,
        }
    }

    /// 露光時間を usec 単位で設定する.
    pub fn with_exposure_time_us(mut self, exposure_time_us: u32) -> Self {
        self.exposure_time_us = Some(exposure_time_us);
        self
    }

    /// アナログゲインを設定する.
    pub fn with_analogue_gain(mut self, analogue_gain: f32) -> Self {
        self.analogue_gain = Some(OrderedF32::new(analogue_gain));
        self
    }

    /// 明るさ補正を設定する.
    pub fn with_brightness(mut self, brightness: f32) -> Self {
        self.brightness = Some(OrderedF32::new(brightness));
        self
    }

    /// コントラスト補正を設定する.
    pub fn with_contrast(mut self, contrast: f32) -> Self {
        self.contrast = Some(OrderedF32::new(contrast));
        self
    }

    /// 彩度補正を設定する.
    pub fn with_saturation(mut self, saturation: f32) -> Self {
        self.saturation = Some(OrderedF32::new(saturation));
        self
    }

    /// シャープネス補正を設定する.
    pub fn with_sharpness(mut self, sharpness: f32) -> Self {
        self.sharpness = Some(OrderedF32::new(sharpness));
        self
    }

    /// 露光時間を usec 単位で返す.
    pub fn exposure_time_us(&self) -> Option<u32> {
        self.exposure_time_us
    }

    /// アナログゲインを返す.
    pub fn analogue_gain(&self) -> Option<f32> {
        self.analogue_gain.map(OrderedF32::get)
    }

    /// 明るさ補正を返す.
    pub fn brightness(&self) -> Option<f32> {
        self.brightness.map(OrderedF32::get)
    }

    /// コントラスト補正を返す.
    pub fn contrast(&self) -> Option<f32> {
        self.contrast.map(OrderedF32::get)
    }

    /// 彩度補正を返す.
    pub fn saturation(&self) -> Option<f32> {
        self.saturation.map(OrderedF32::get)
    }

    /// シャープネス補正を返す.
    pub fn sharpness(&self) -> Option<f32> {
        self.sharpness.map(OrderedF32::get)
    }
}

impl OrderedF32 {
    fn new(value: f32) -> Self {
        Self(value.to_bits())
    }

    fn get(self) -> f32 {
        f32::from_bits(self.0)
    }
}

impl LibcameraSource {
    /// `libcamerasrc` 用の既定設定を返す.
    pub fn new() -> Self {
        Self { camera_name: None }
    }

    /// 利用するカメラ名を設定する.
    pub fn with_camera_name(mut self, camera_name: impl Into<String>) -> Self {
        self.camera_name = Some(camera_name.into());
        self
    }

    /// 利用するカメラ名を返す.
    pub fn camera_name(&self) -> Option<&str> {
        self.camera_name.as_deref()
    }
}

impl V4l2Source {
    /// `v4l2src` 用の既定設定を返す.
    pub fn new() -> Self {
        Self { device_path: None }
    }

    /// 利用するデバイスパスを設定する.
    pub fn with_device_path(mut self, device_path: impl Into<String>) -> Self {
        self.device_path = Some(device_path.into());
        self
    }

    /// 利用するデバイスパスを返す.
    pub fn device_path(&self) -> Option<&str> {
        self.device_path.as_deref()
    }
}

impl VideoTestSource {
    /// `videotestsrc` 用の既定設定を返す.
    pub fn new() -> Self {
        Self {
            is_live: true,
            pattern: None,
        }
    }

    /// live source として扱うかを設定する.
    pub fn with_is_live(mut self, is_live: bool) -> Self {
        self.is_live = is_live;
        self
    }

    /// `videotestsrc` の pattern を設定する.
    pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    /// live source として扱うかを返す.
    pub fn is_live(&self) -> bool {
        self.is_live
    }

    /// `videotestsrc` の pattern を返す.
    pub fn pattern(&self) -> Option<&str> {
        self.pattern.as_deref()
    }
}

/// 配信開始に必要な設定を保持する型.
///
/// `new()` で基本設定を作り, `with_...()` 系メソッドで必要な項目を上書きして利用する.
///
/// `new()` 直後の主な既定値は以下の通り.
///
/// - `source`: `StreamSource::Imx500`
/// - `width`: `1280`
/// - `height`: `720`
/// - `framerate`: `20`
/// - `bitrate`: `2_000_000`
///
/// # Examples
///
/// ```
/// use raspi_stream::{Imx500Source, Imx500Tuning, StreamConfig, StreamSource};
///
/// let config = StreamConfig::new("0.0.0.0", 8554)
///     .with_resolution(1280, 720)
///     .with_framerate(20)
///     .with_bitrate(2_000_000)
///     .with_stream_path("/camera")
///     .with_source(StreamSource::Imx500(
///         Imx500Source::new()
///             .with_camera_name("/base/soc/i2c0mux/i2c@1/imx500@1a")
///             .with_tuning(Imx500Tuning::new().with_exposure_time_us(10_000)),
///     ));
///
/// assert_eq!(config.bind_host(), "0.0.0.0");
/// assert_eq!(config.listen_port(), 8554);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamConfig {
    /// 入力ソース設定.
    source: StreamSource,
    /// RTSP server の bind 先ホスト名または IP アドレス.
    bind_host: String,
    /// RTSP server の listen ポート番号.
    listen_port: u16,
    /// RTSP mount path.
    stream_path: String,
    /// 映像の幅. 単位は pixel.
    width: u32,
    /// 映像の高さ. 単位は pixel.
    height: u32,
    /// フレームレート. 単位は fps.
    framerate: u32,
    /// ビットレート. 単位は bps.
    bitrate: u32,
}

impl StreamConfig {
    /// 最低限必要な配信先情報から設定を生成する.
    /// `bind_host` は RTSP server の bind 先ホスト名または IP アドレス,
    /// `listen_port` は listen ポート番号を表す.
    ///
    /// それ以外の既定値は以下の通り.
    ///
    /// - 入力ソース: IMX500 用 `libcamerasrc`
    /// - RTSP path: `/stream`
    /// - 解像度: `1280x720`
    /// - フレームレート: `20fps`
    /// - ビットレート: `2_000_000bps`
    pub fn new(bind_host: impl Into<String>, listen_port: u16) -> Self {
        Self {
            source: StreamSource::imx500(),
            bind_host: bind_host.into(),
            listen_port,
            stream_path: "/stream".to_string(),
            width: 1280,
            height: 720,
            framerate: 20,
            bitrate: 2_000_000,
        }
    }

    /// 入力ソースを上書きする.
    ///
    /// Raspberry Pi AI Camera では [`StreamSource::Imx500`], それ以外の
    /// `libcamera` 系入力では [`StreamSource::Libcamera`], USB カメラでは
    /// [`StreamSource::V4l2`], WSL 上の開発では [`StreamSource::VideoTest`] を想定する.
    pub fn with_source(mut self, source: StreamSource) -> Self {
        self.source = source;
        self
    }

    /// RTSP mount path を上書きする.
    ///
    /// `/stream` のように `/` から始まる path を想定する.
    pub fn with_stream_path(mut self, stream_path: impl Into<String>) -> Self {
        self.stream_path = stream_path.into();
        self
    }

    /// 解像度を上書きする.
    ///
    /// `width` と `height` はどちらも `1` 以上が必要.
    /// 妥当性確認は [`StreamConfig::validate()`] で行う.
    pub fn with_resolution(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// フレームレートを上書きする.
    ///
    /// 単位は fps. `1` 以上が必要.
    pub fn with_framerate(mut self, framerate: u32) -> Self {
        self.framerate = framerate;
        self
    }

    /// ビットレートを上書きする.
    ///
    /// 単位は bps. `1` 以上が必要.
    pub fn with_bitrate(mut self, bitrate: u32) -> Self {
        self.bitrate = bitrate;
        self
    }

    /// 設定値が配信開始可能な範囲か検証する.
    ///
    /// 主に以下を検証する.
    ///
    /// - `bind_host` が空文字ではないこと
    /// - `listen_port` が `1` 以上であること
    /// - `width`, `height`, `framerate`, `bitrate` が `1` 以上であること
    ///
    /// # Errors
    ///
    /// 次の場合に [`StreamError::InvalidConfig`] を返す.
    ///
    /// - `imx500` の tuning に含まれる analogue gain が `0` 以下
    /// - `imx500` の tuning に含まれる brightness, contrast, saturation,
    ///   sharpness のいずれかが非 finite
    /// - `imx500` の tuning に含まれる exposure time が `0`
    /// - `imx500` の camera name を明示指定した場合に空文字または空白のみ
    /// - `libcamerasrc` の camera name が空文字または空白のみ
    /// - `bind_host` が空文字または空白のみ
    /// - `listen_port` が `0`
    /// - `stream_path` が空文字または空白のみ
    /// - `stream_path` が `/` で始まらない
    /// - `v4l2src` の device path が空文字または空白のみ
    /// - `width`, `height`, `framerate`, `bitrate` のいずれかが `0`
    /// - `videotestsrc` の pattern が空文字または空白のみ
    pub fn validate(&self) -> Result<(), StreamError> {
        match &self.source {
            StreamSource::Imx500(source) => {
                if let Some(camera_name) = source.camera_name()
                    && camera_name.trim().is_empty()
                {
                    return Err(StreamError::InvalidConfig(
                        "imx500 camera_name must not be empty".to_string(),
                    ));
                }

                let tuning = source.tuning();

                if matches!(tuning.exposure_time_us(), Some(0)) {
                    return Err(StreamError::InvalidConfig(
                        "imx500 exposure_time_us must be greater than zero".to_string(),
                    ));
                }

                if matches!(tuning.analogue_gain(), Some(value) if !value.is_finite() || value <= 0.0)
                {
                    return Err(StreamError::InvalidConfig(
                        "imx500 analogue_gain must be finite and greater than zero".to_string(),
                    ));
                }

                for (name, value) in [
                    ("brightness", tuning.brightness()),
                    ("contrast", tuning.contrast()),
                    ("saturation", tuning.saturation()),
                    ("sharpness", tuning.sharpness()),
                ] {
                    if matches!(value, Some(value) if !value.is_finite()) {
                        return Err(StreamError::InvalidConfig(format!(
                            "imx500 {name} must be finite"
                        )));
                    }
                }
            }
            StreamSource::Libcamera(source) => {
                if let Some(camera_name) = source.camera_name()
                    && camera_name.trim().is_empty()
                {
                    return Err(StreamError::InvalidConfig(
                        "libcamera camera_name must not be empty".to_string(),
                    ));
                }
            }
            StreamSource::V4l2(source) => {
                if let Some(device_path) = source.device_path()
                    && device_path.trim().is_empty()
                {
                    return Err(StreamError::InvalidConfig(
                        "v4l2 device_path must not be empty".to_string(),
                    ));
                }
            }
            StreamSource::VideoTest(source) => {
                if let Some(pattern) = source.pattern()
                    && pattern.trim().is_empty()
                {
                    return Err(StreamError::InvalidConfig(
                        "videotest pattern must not be empty".to_string(),
                    ));
                }
            }
        }

        if self.bind_host.trim().is_empty() {
            return Err(StreamError::InvalidConfig(
                "bind_host must not be empty".to_string(),
            ));
        }

        if self.listen_port == 0 {
            return Err(StreamError::InvalidConfig(
                "listen_port must be greater than zero".to_string(),
            ));
        }

        if self.stream_path.trim().is_empty() {
            return Err(StreamError::InvalidConfig(
                "stream_path must not be empty".to_string(),
            ));
        }

        if !self.stream_path.starts_with('/') {
            return Err(StreamError::InvalidConfig(
                "stream_path must start with '/'".to_string(),
            ));
        }

        if self.width == 0 {
            return Err(StreamError::InvalidConfig(
                "width must be greater than zero".to_string(),
            ));
        }

        if self.height == 0 {
            return Err(StreamError::InvalidConfig(
                "height must be greater than zero".to_string(),
            ));
        }

        if self.framerate == 0 {
            return Err(StreamError::InvalidConfig(
                "framerate must be greater than zero".to_string(),
            ));
        }

        if self.bitrate == 0 {
            return Err(StreamError::InvalidConfig(
                "bitrate must be greater than zero".to_string(),
            ));
        }

        Ok(())
    }

    /// 入力ソース設定を返す.
    pub fn source(&self) -> &StreamSource {
        &self.source
    }

    /// RTSP server の bind 先ホスト名または IP アドレスを返す.
    pub fn bind_host(&self) -> &str {
        &self.bind_host
    }

    /// RTSP server の listen ポート番号を返す.
    pub fn listen_port(&self) -> u16 {
        self.listen_port
    }

    /// RTSP mount path を返す.
    pub fn stream_path(&self) -> &str {
        &self.stream_path
    }

    /// 映像の幅を返す.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// 映像の高さを返す.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// フレームレートを fps 単位で返す.
    pub fn framerate(&self) -> u32 {
        self.framerate
    }

    /// ビットレートを bps 単位で返す.
    pub fn bitrate(&self) -> u32 {
        self.bitrate
    }
}

impl Default for LibcameraSource {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Imx500Source {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Imx500Tuning {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for V4l2Source {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for VideoTestSource {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Imx500Source, Imx500Tuning, LibcameraSource, StreamConfig, StreamError, StreamSource,
        V4l2Source, VideoTestSource,
    };

    #[test]
    fn new_sets_defaults() {
        let config = StreamConfig::new("192.168.1.10", 5000);

        assert_eq!(config.source(), &StreamSource::Imx500(Imx500Source::new()));
        assert_eq!(config.bind_host(), "192.168.1.10");
        assert_eq!(config.listen_port(), 5000);
        assert_eq!(config.stream_path(), "/stream");
        assert_eq!(config.width(), 1280);
        assert_eq!(config.height(), 720);
        assert_eq!(config.framerate(), 20);
        assert_eq!(config.bitrate(), 2_000_000);
    }

    #[test]
    fn libcamera_source_has_defaults() {
        let source = LibcameraSource::new();

        assert_eq!(source.camera_name(), None);
    }

    #[test]
    fn imx500_source_has_defaults() {
        let source = Imx500Source::new();

        assert_eq!(source.camera_name(), None);
        assert_eq!(source.tuning(), &Imx500Tuning::new());
    }

    #[test]
    fn imx500_tuning_builder_methods_store_typed_settings() {
        let tuning = Imx500Tuning::new()
            .with_exposure_time_us(12_000)
            .with_analogue_gain(2.5)
            .with_brightness(0.1)
            .with_contrast(1.2)
            .with_saturation(1.1)
            .with_sharpness(0.8);

        assert_eq!(tuning.exposure_time_us(), Some(12_000));
        assert_eq!(tuning.analogue_gain(), Some(2.5));
        assert_eq!(tuning.brightness(), Some(0.1));
        assert_eq!(tuning.contrast(), Some(1.2));
        assert_eq!(tuning.saturation(), Some(1.1));
        assert_eq!(tuning.sharpness(), Some(0.8));
    }

    #[test]
    fn imx500_source_can_wrap_tuning() {
        let tuning = Imx500Tuning::new().with_exposure_time_us(12_000);
        let source = Imx500Source::new().with_tuning(tuning.clone());

        assert_eq!(source.tuning(), &tuning);
    }

    #[test]
    fn videotest_source_has_defaults() {
        let source = VideoTestSource::new();

        assert!(source.is_live());
        assert_eq!(source.pattern(), None);
    }

    #[test]
    fn v4l2_source_has_defaults() {
        let source = V4l2Source::new();

        assert_eq!(source.device_path(), None);
    }

    #[test]
    fn builder_methods_override_defaults() {
        let config = StreamConfig::new("192.168.1.10", 5000)
            .with_source(StreamSource::Imx500(
                Imx500Source::new().with_camera_name("imx500-main"),
            ))
            .with_stream_path("/camera")
            .with_resolution(640, 480)
            .with_framerate(15)
            .with_bitrate(1_000_000);

        assert_eq!(
            config.source(),
            &StreamSource::Imx500(Imx500Source::new().with_camera_name("imx500-main"))
        );
        assert_eq!(config.stream_path(), "/camera");
        assert_eq!(config.width(), 640);
        assert_eq!(config.height(), 480);
        assert_eq!(config.framerate(), 15);
        assert_eq!(config.bitrate(), 1_000_000);
    }

    #[test]
    fn builder_methods_override_defaults_for_libcamera() {
        let config = StreamConfig::new("192.168.1.10", 5000)
            .with_source(StreamSource::Libcamera(
                LibcameraSource::new().with_camera_name("ov5647"),
            ))
            .with_resolution(640, 480)
            .with_framerate(15)
            .with_bitrate(1_000_000);

        assert_eq!(
            config.source(),
            &StreamSource::Libcamera(LibcameraSource::new().with_camera_name("ov5647"))
        );
        assert_eq!(config.width(), 640);
        assert_eq!(config.height(), 480);
        assert_eq!(config.framerate(), 15);
        assert_eq!(config.bitrate(), 1_000_000);
    }

    #[test]
    fn builder_methods_override_defaults_for_videotest() {
        let config = StreamConfig::new("192.168.1.10", 5000)
            .with_source(StreamSource::VideoTest(
                VideoTestSource::new().with_pattern("ball"),
            ))
            .with_resolution(640, 480)
            .with_framerate(15)
            .with_bitrate(1_000_000);

        assert_eq!(
            config.source(),
            &StreamSource::VideoTest(VideoTestSource::new().with_pattern("ball"))
        );
        assert_eq!(config.width(), 640);
        assert_eq!(config.height(), 480);
        assert_eq!(config.framerate(), 15);
        assert_eq!(config.bitrate(), 1_000_000);
    }

    #[test]
    fn builder_methods_override_defaults_for_v4l2() {
        let config = StreamConfig::new("192.168.1.10", 5000)
            .with_source(StreamSource::V4l2(
                V4l2Source::new().with_device_path("/dev/video2"),
            ))
            .with_resolution(640, 480)
            .with_framerate(15)
            .with_bitrate(1_000_000);

        assert_eq!(
            config.source(),
            &StreamSource::V4l2(V4l2Source::new().with_device_path("/dev/video2"))
        );
        assert_eq!(config.width(), 640);
        assert_eq!(config.height(), 480);
        assert_eq!(config.framerate(), 15);
        assert_eq!(config.bitrate(), 1_000_000);
    }

    #[test]
    fn validate_rejects_empty_libcamera_camera_name() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::Libcamera(
            LibcameraSource::new().with_camera_name("   "),
        ));

        assert_eq!(
            config.validate(),
            Err(StreamError::InvalidConfig(
                "libcamera camera_name must not be empty".to_string(),
            ))
        );
    }

    #[test]
    fn validate_rejects_empty_host() {
        let config = StreamConfig::new("   ", 5000);

        assert_eq!(
            config.validate(),
            Err(StreamError::InvalidConfig(
                "bind_host must not be empty".to_string(),
            ))
        );
    }

    #[test]
    fn validate_rejects_empty_stream_path() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_stream_path("   ");

        assert_eq!(
            config.validate(),
            Err(StreamError::InvalidConfig(
                "stream_path must not be empty".to_string(),
            ))
        );
    }

    #[test]
    fn validate_rejects_stream_path_without_leading_slash() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_stream_path("camera");

        assert_eq!(
            config.validate(),
            Err(StreamError::InvalidConfig(
                "stream_path must start with '/'".to_string(),
            ))
        );
    }

    #[test]
    fn validate_rejects_empty_videotest_pattern() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::VideoTest(
            VideoTestSource::new().with_pattern("   "),
        ));

        assert_eq!(
            config.validate(),
            Err(StreamError::InvalidConfig(
                "videotest pattern must not be empty".to_string(),
            ))
        );
    }

    #[test]
    fn validate_rejects_empty_v4l2_device_path() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::V4l2(
            V4l2Source::new().with_device_path("   "),
        ));

        assert_eq!(
            config.validate(),
            Err(StreamError::InvalidConfig(
                "v4l2 device_path must not be empty".to_string(),
            ))
        );
    }

    #[test]
    fn validate_rejects_zero_values() {
        let config = StreamConfig::new("127.0.0.1", 5000)
            .with_resolution(0, 720)
            .with_framerate(0)
            .with_bitrate(0);

        assert_eq!(
            config.validate(),
            Err(StreamError::InvalidConfig(
                "width must be greater than zero".to_string(),
            ))
        );
    }

    #[test]
    fn validate_rejects_empty_imx500_camera_name() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::Imx500(
            Imx500Source::new().with_camera_name("   "),
        ));

        assert_eq!(
            config.validate(),
            Err(StreamError::InvalidConfig(
                "imx500 camera_name must not be empty".to_string(),
            ))
        );
    }

    #[test]
    fn validate_rejects_zero_imx500_exposure_time() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::Imx500(
            Imx500Source::new().with_tuning(Imx500Tuning::new().with_exposure_time_us(0)),
        ));

        assert_eq!(
            config.validate(),
            Err(StreamError::InvalidConfig(
                "imx500 exposure_time_us must be greater than zero".to_string(),
            ))
        );
    }

    #[test]
    fn validate_rejects_non_positive_imx500_analogue_gain() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::Imx500(
            Imx500Source::new().with_tuning(Imx500Tuning::new().with_analogue_gain(0.0)),
        ));

        assert_eq!(
            config.validate(),
            Err(StreamError::InvalidConfig(
                "imx500 analogue_gain must be finite and greater than zero".to_string(),
            ))
        );
    }

    #[test]
    fn validate_rejects_non_finite_imx500_brightness() {
        let config = StreamConfig::new("127.0.0.1", 5000).with_source(StreamSource::Imx500(
            Imx500Source::new().with_tuning(Imx500Tuning::new().with_brightness(f32::NAN)),
        ));

        assert_eq!(
            config.validate(),
            Err(StreamError::InvalidConfig(
                "imx500 brightness must be finite".to_string(),
            ))
        );
    }
}
