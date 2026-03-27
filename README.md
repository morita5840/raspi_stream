# raspi_stream

raspi_stream は, Raspberry Pi 向けの RTSP 映像配信ライブラリ / アプリケーションです.

Rust から GStreamer と `gst-rtsp-server` を制御し, 複数のカメラ入力ソースを共通の設定モデルで扱いながら RTSP サーバを起動できます.

主な実行対象は Raspberry Pi ですが, ホスト側でのテストや aarch64 向けクロスビルドにも対応しています.

## 主な機能

- Raspberry Pi 上で RTSP ストリームを手早く立ち上げる
- `imx500`, `libcamera`, `v4l2`, `videotest` を同じ CLI / ライブラリ API で扱う
- 起動時のパイプライン候補を検査し, 失敗理由を診断付きで確認する
- Rust のライブラリとして組み込み, アプリケーションから配信制御する

## 主な特徴

- CLI とライブラリの両方を提供
- GStreamer ベースの RTSP 配信
- 複数ソースを共通設定で切り替え可能
- 起動診断とフォールバック情報を取得可能
- Raspberry Pi 向けクロスビルドを考慮した構成

## 対応ソース

現時点で以下の入力ソースをサポートしています.

- `imx500`: Raspberry Pi AI Camera (IMX500)
- `libcamera`: libcamera 対応カメラ（Pi Camera など）
- `v4l2`: `/dev/video*` デバイス（USB カメラなど）
- `videotest`: 開発用ダミー映像

## クイックスタート

最短で確認するだけなら, `videotest` で起動して RTSP クライアントから接続するのが簡単です.

### 1. 依存パッケージを入れる

まず, 実行に必要なランタイムとビルドツールを用意します.

```bash
sudo apt update
sudo apt install -y libgstrtspserver-1.0-0 gstreamer1.0-plugins-ugly build-essential pkg-config

# Rust ツールチェインが未導入の場合のみ実行
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

カメラソースを使う場合は, 追加で以下を入れます.

```bash
# libcamera ベースのカメラ
sudo apt install -y gstreamer1.0-libcamera

# IMX500 AI Camera を使う場合
sudo apt install -y gstreamer1.0-libcamera imx500-all
```

### 2. リポジトリを取得する

```bash
git clone https://github.com/morita5840/raspi_stream.git
cd raspi_stream
```

### 3. まずは `videotest` で起動する

最初の確認はダミー映像が最も確実です.

```bash
cargo run --release -- --source videotest --pattern ball --host 127.0.0.1 --port 8554
```

ローカル端末からは以下に接続します.

```text
rtsp://127.0.0.1:8554/stream
```

別マシンから接続したい場合は, Pi 側を `0.0.0.0` でバインドします.

```bash
cargo run --release -- --source videotest --pattern ball --host 0.0.0.0 --port 8554
```

その場合の接続先は以下です.

```text
rtsp://<pi-ip>:8554/stream
```

### 4. 実カメラで起動する

手元のデバイスに応じてソースを切り替えます.

```bash
# IMX500 実機カメラ
cargo run --release -- --source imx500 --host 127.0.0.1 --port 8554

# libcamera 対応カメラ
cargo run --release -- --source libcamera --host 127.0.0.1 --port 8554

# USB カメラ
cargo run --release -- --source v4l2 --host 127.0.0.1 --port 8554
```

起動時に失敗理由を確認したい場合は `--verbose` を付けます.

```bash
cargo run --release -- --source v4l2 --verbose --host 127.0.0.1 --port 8554
```

### 5. クライアントで確認する

- VLC などで `rtsp://127.0.0.1:8554/stream` に接続します.
- リモート接続時は `rtsp://<pi-ip>:8554/stream` を使います.

### 6. 停止する

- Ctrl+C で停止します.

詳細なオプション一覧は `cargo run -- --help` を参照してください.
 
## 起動オプション

以下はよく使うオプションの概要です. 詳細は `cargo run -- --help` を参照してください.

- ネットワーク
  - `--host`: RTSP サーバが待ち受けるアドレスです. 既定は `127.0.0.1` です. 別マシンから接続する場合は `--host 0.0.0.0` を指定します.
  - `--port`: RTSP サーバが待ち受けるポート番号です. 既定は `8554` です. 例: `--port 8554`
  - `--path`: RTSP のマウントパスです. 既定は `/stream` です. 例: `--path /camera`
  - `--verbose`: 起動時に試行した候補や失敗理由を詳しく表示します. 通常時は短い要約のみ表示します.

- ソース選択
  - `--source`: 使用する入力ソースを指定します. `auto | imx500 | libcamera | v4l2 | videotest` のいずれかを選べます. 例: `--source imx500`

- ソース`videotest` 専用
  - `--pattern`: テスト映像のパターンを指定します. 既定は `ball` です. 例: `--pattern ball`

- ソース`v4l2` 専用
  - `--device-path`: 使用するデバイスノードを明示したい場合に指定します. 例: `--device-path /dev/video0`

- ソース`libcamera` 専用
  - `--camera-name`: 複数カメラ環境などで特定のカメラを選びたい場合に指定します. libcamera が列挙する camera ID を使います.

- ソース`imx500` 専用
  - `--camera-name`: 複数カメラ環境などで特定のカメラを選びたい場合に指定します. libcamera が列挙する camera ID を使います.
  - `--exposure-time-us`: 露光時間をマイクロ秒で指定します. 例: `--exposure-time-us 10000`
  - `--analogue-gain`: アナログゲインを指定します. 例: `--analogue-gain 2.0`
  - `--brightness`, `--contrast`, `--saturation`, `--sharpness`: 画質調整パラメータを数値で指定します.

- 映像設定
  - `--width`: 横方向の解像度です. 既定は `1280` です. 例: `--width 1280`
  - `--height`: 縦方向の解像度です. 既定は `720` です. 例: `--height 720`
  - `--framerate`: フレームレートです. 既定は `20` です. 例: `--framerate 20`
  - `--bitrate`: エンコード時のビットレートです. 既定は `2000000` です. 例: `--bitrate 2000000`

## クロスビルド環境

Raspberry Pi 向けの aarch64 バイナリは, Linux ホスト上でクロスビルドできます.

### 1. 必要パッケージを入れる

クロスビルドには以下が必要です.

```bash
sudo apt install gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu pkg-config
```

ホスト側で開発やテストも行う場合は, 追加でホスト向け GStreamer 開発パッケージを入れます.

```bash
sudo apt install libgstrtspserver-1.0-dev build-essential pkg-config
```

### 2. Rust ターゲットを追加する

```bash
rustup target add aarch64-unknown-linux-gnu
```

### 3. ビルドする

デバッグビルド:

```bash
cargo build --target aarch64-unknown-linux-gnu
```

リリースビルド:

```bash
cargo build --release --target aarch64-unknown-linux-gnu
```

### デプロイ

`scripts/deploy-rpi.sh` を使うと, aarch64 向けにビルドしたバイナリを Raspberry Pi へ転送できます.

前提:

- `ssh` と `scp` が使えること
- Raspberry Pi へ SSH 接続できること
- 既定では `pi` ユーザー, `/home/pi/raspi_stream` に配置すること

基本例:

```bash
scripts/deploy-rpi.sh --host raspberrypi.local
```

配置先を変える場合:

```bash
scripts/deploy-rpi.sh --host raspberrypi.local --remote-dir /opt/raspi_stream
```

転送後にそのまま起動する場合:

```bash
scripts/deploy-rpi.sh --host raspberrypi.local --run -- --source libcamera --host 0.0.0.0 --path /camera
```

`--run` を付けると, 転送後に SSH 経由でそのままバイナリを起動します.


## ライブラリ利用例

README では, まず最小の利用例として `videotest` を使った構成を示します.

```rust
use std::time::Duration;

use raspi_stream::{CameraStreamer, StreamConfig, StreamEvent, StreamSource};

let config = StreamConfig::new("0.0.0.0", 8554)
  .with_stream_path("/stream")
  .with_source(StreamSource::videotest());

let session = CameraStreamer::new(config).start()?;

if let Some(StreamEvent::Started { stream_url }) =
  session.poll_event(Duration::from_millis(100))
{
  println!("stream ready: {stream_url}");
}

session.stop()?;
# Ok::<(), raspi_stream::StreamError>(())
```

実カメラを使う場合は, `with_source()` を `StreamSource::imx500()`, `StreamSource::libcamera()`, `StreamSource::v4l2()` などに切り替えます.

起動後は `poll_event()` で主に以下のイベントを受け取れます.

- `Started`: RTSP サーバが起動し, URL を返せる状態になった
- `Error`: ランタイム内で復旧不能な異常が起きた
- `Stopped`: 明示停止または異常終了でセッションが終了した

## テスト

このリポジトリはクロス向け設定を持っているため, ホストでのテスト実行時はターゲットを明示します.

```bash
cargo test --target x86_64-unknown-linux-gnu
```

## 注意点

- `imx500`, `libcamera`, `v4l2`, `videotest` で内部のパイプライン構成は同一ではありません.
- `imx500`, `libcamera`, `v4l2` は H.264 系の配信経路を使いますが, `videotest` は `vp8enc` と `rtpvp8pay` を使います.
- カメラソースの起動可否は, 接続されているデバイス, GStreamer プラグイン, 対応フォーマットの組み合わせに依存します.
- 起動に失敗した場合は, まず `--verbose` を付けて試行した候補と失敗理由を確認してください.
