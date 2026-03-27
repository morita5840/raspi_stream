/// 公開 API が返すエラー.
///
/// 各 variant が保持する文字列は, ログ表示や診断のための人間向けメッセージを表す.
/// 安定した識別子として比較する用途は想定しない.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    /// 設定値が妥当でない場合のエラー.
    InvalidConfig(String),
    /// GStreamer 初期化に失敗した場合のエラー.
    InitFailed(String),
    /// 配信用 launch 文字列の構築に失敗した場合のエラー.
    PipelineBuildFailed(String),
    /// 実行中パイプラインの状態変更に失敗した場合のエラー.
    StateChangeFailed(String),
    /// RTSP server 実行中に発生したその他のエラー.
    RuntimeError(String),
}
