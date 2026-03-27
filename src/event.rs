/// 実行中配信セッションから取得できるイベント.
///
/// [`crate::StreamSession::poll_event()`] で逐次取り出して扱う.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// RTSP server の起動完了イベント.
    ///
    /// [`crate::CameraStreamer::start()`] が成功し, source と配信 pipeline の起動確認まで通って,
    /// クライアントを受け付けられる状態になった後に最初のイベントとして返る.
    Started {
        /// bind_host ベースの RTSP URL.
        /// bind_host が `0.0.0.0` や `::` の場合は, 利用側で到達可能な実アドレスへ読み替える.
        stream_url: String,
    },
    /// 実行継続可能な警告イベント.
    ///
    /// 現在の RTSP runtime では通常発生しないが, 将来的に GStreamer の warning を
    /// そのまま利用者へ通知するためのイベント.
    Warning {
        /// 警告を発生させた GStreamer 要素名.
        source: String,
        /// 警告内容.
        message: String,
    },
    /// 実行中の致命的なエラーイベント.
    ///
    /// セッション開始後に RTSP runtime 内で復旧不能な異常が起きたときに返る.
    /// このイベントの後には通常 [`StreamEvent::Stopped`] が続く.
    Error {
        /// エラーを発生させた GStreamer 要素名.
        source: String,
        /// エラー内容.
        message: String,
    },
    /// RTSP server の停止イベント.
    ///
    /// [`crate::StreamSession::stop()`] による明示停止, または runtime 側の異常終了後に返る.
    Stopped {
        /// 停止理由. 明示停止時は `None`, 異常終了時は理由文字列を持つ.
        reason: Option<String>,
    },
    /// ストリーム終端イベント.
    ///
    /// 将来的な GStreamer EOS 伝播のために予約しているイベント.
    /// 現在の RTSP runtime では通常発生しない.
    EndOfStream,
}
