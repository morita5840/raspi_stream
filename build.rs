//! Raspberry Pi Zero 2 W 向けの arm64 sysroot を自動取得・更新する build script.
//!
//! この build script は以下を行う.
//! - 最新の Raspberry Pi OS Lite (arm64) の SHA256 を取得
//! - `vendor/rpi-sysroot/version.txt` と比較
//! - 差分があればイメージを再開可能ダウンロード
//! - SHA256 を検証
//! - `.img.xz` を展開
//! - `parted`, `dd`, `debugfs` を使って `/usr`, `/lib`, `/opt` を sysroot として抽出
//! - 必要なら symlink を相対化
//! - `version.txt` を更新

use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, anyhow, bail};
use pathdiff::diff_paths;
use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_LENGTH, RANGE};
use sha2::{Digest, Sha256};
use tempfile::Builder;
use walkdir::WalkDir;
use xz2::read::XzDecoder;

const IMAGE_URL: &str = "https://downloads.raspberrypi.com/raspios_lite_arm64_latest";
const SHA256_URL: &str = "https://downloads.raspberrypi.com/raspios_lite_arm64_latest.sha256";
const VERSION_FILE_NAME: &str = "version.txt";
const RPI_TARGET_TRIPLE: &str = "aarch64-unknown-linux-gnu";
const DEFAULT_DEBIAN_SUITE: &str = "bookworm";
const DEBIAN_MIRROR_URL: &str = "https://deb.debian.org/debian";
const REQUIRED_SYSROOT_PACKAGES: &[&str] = &[
    "libblkid-dev",
    "libdw-dev",
    "libelf-dev",
    "libffi-dev",
    "libglib2.0-dev",
    "liblzma-dev",
    "libmount-dev",
    "liborc-0.4-0",
    "liborc-0.4-dev",
    "libselinux1-dev",
    "libsepol-dev",
    "libunwind8",
    "libunwind-dev",
    "libpcre2-dev",
    "libgstreamer1.0-0",
    "libgstreamer1.0-dev",
    "libgstreamer-plugins-base1.0-0",
    "libgstreamer-plugins-base1.0-dev",
    "libgstrtspserver-1.0-0",
    "libgstrtspserver-1.0-dev",
    "zlib1g-dev",
];
const CROSS_PKG_CONFIG_PROBE_MODULES: &[&str] = &["glib-2.0", "gstreamer-rtsp-server-1.0"];

struct DebianPackage {
    filename: String,
    sha256: Option<String>,
}

/// build script のエントリポイント.
fn main() {
    if is_raspberry_pi() {
        println!("cargo:warning=Running on Raspberry Pi; skipping sysroot setup");
        return;
    }
    if let Err(err) = run() {
        panic!("failed to prepare Raspberry Pi sysroot: {err:#}");
    }
}

/// Raspberry Pi 上でビルドしているかを判定する.
fn is_raspberry_pi() -> bool {
    if let Ok(model) = std::fs::read_to_string("/proc/device-tree/model") {
        return model.contains("Raspberry Pi");
    }
    false
}

/// sysroot の更新要否を判定し、必要であれば更新処理を実行する.
fn run() -> Result<()> {
    emit_rerun_hints();

    if !should_prepare_sysroot()? {
        return Ok(());
    }

    if env_flag("RPI_SYSROOT_SKIP") {
        println!("cargo:warning=Skipping Raspberry Pi sysroot setup because RPI_SYSROOT_SKIP=1");
        return Ok(());
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let vendor_dir = manifest_dir.join("vendor");
    let sysroot_dir = vendor_dir.join("rpi-sysroot");
    let version_file = sysroot_dir.join(VERSION_FILE_NAME);

    fs::create_dir_all(&vendor_dir)
        .with_context(|| format!("failed to create {}", vendor_dir.display()))?;

    let client = build_http_client()?;
    let latest_sha = fetch_latest_sha256(&client)?;

    if !env_flag("RPI_SYSROOT_FORCE_CHECK")
        && is_sysroot_current(&sysroot_dir, &version_file, &latest_sha)?
    {
        ensure_cross_build_packages(&vendor_dir, &sysroot_dir, &client)?;
        return Ok(());
    }

    println!("cargo:warning=Updating Raspberry Pi arm64 sysroot to {latest_sha}");
    update_sysroot(
        &manifest_dir,
        &vendor_dir,
        &sysroot_dir,
        &latest_sha,
        &client,
    )?;
    ensure_cross_build_packages(&vendor_dir, &sysroot_dir, &client)?;
    println!("cargo:warning=Raspberry Pi arm64 sysroot updated successfully");
    Ok(())
}

/// Cargo に対して再実行条件を通知する.
fn emit_rerun_hints() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=vendor/rpi-sysroot/version.txt");
    println!("cargo:rerun-if-env-changed=RPI_SYSROOT_SKIP");
    println!("cargo:rerun-if-env-changed=RPI_SYSROOT_FORCE_CHECK");
    println!("cargo:rerun-if-env-changed=RPI_SYSROOT_DEBIAN_SUITE");
}

/// 現在の Cargo target で sysroot 準備が必要かを判定する.
fn should_prepare_sysroot() -> Result<bool> {
    let target = env::var("TARGET").context("TARGET is not set")?;
    Ok(target == RPI_TARGET_TRIPLE)
}

/// 環境変数を真偽値フラグとして解釈する.
///
/// `1`, `true`, `TRUE` を真として扱う.
fn env_flag(name: &str) -> bool {
    env::var_os(name)
        .map(|value| value == "1" || value == "true" || value == "TRUE")
        .unwrap_or(false)
}

/// Raspberry Pi ダウンロードサーバへ接続するための HTTP クライアントを生成する.
///
/// # Returns
/// `reqwest::blocking::Client` を返す.
fn build_http_client() -> Result<Client> {
    Client::builder()
        .user_agent("raspi_stream-build-script/0.1")
        .build()
        .context("failed to build HTTP client")
}

/// Raspberry Pi OS Lite (arm64) 最新イメージの SHA256 を取得する.
///
/// # Arguments
/// * `client` - SHA256 配布 URL へアクセスする HTTP クライアント.
///
/// # Returns
/// 64 文字の小文字 SHA256 文字列を返す.
fn fetch_latest_sha256(client: &Client) -> Result<String> {
    let body = client
        .get(SHA256_URL)
        .send()
        .context("failed to download SHA256 file")?
        .error_for_status()
        .context("SHA256 endpoint returned an error")?
        .text()
        .context("failed to read SHA256 response body")?;

    let sha = body
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow!("unexpected SHA256 response format: {body}"))?;

    if sha.len() != 64 || !sha.chars().all(|ch| ch.is_ascii_hexdigit()) {
        bail!("invalid SHA256 value returned by Raspberry Pi download server: {sha}");
    }

    Ok(sha.to_ascii_lowercase())
}

/// 現在の sysroot が最新 SHA256 と一致し、最低限のディレクトリを持つか確認する.
///
/// # Arguments
/// * `sysroot_dir` - 検査対象の sysroot ルートディレクトリ.
/// * `version_file` - 保存済み SHA256 を保持する `version.txt` のパス.
/// * `latest_sha` - 配布元から取得した最新 SHA256.
///
/// # Returns
/// sysroot が最新状態なら `true` を返す.
fn is_sysroot_current(sysroot_dir: &Path, version_file: &Path, latest_sha: &str) -> Result<bool> {
    if !sysroot_dir.exists() || !version_file.exists() {
        return Ok(false);
    }

    let stored_sha = fs::read_to_string(version_file)
        .with_context(|| format!("failed to read {}", version_file.display()))?;

    let required_dirs = [sysroot_dir.join("usr"), sysroot_dir.join("lib")];
    Ok(stored_sha.trim() == latest_sha && required_dirs.iter().all(|path| path.exists()))
}

/// sysroot を更新する.
///
/// キャッシュ済みアーカイブの再利用、イメージ展開、`parted` / `dd` / `debugfs` による抽出、
/// symlink 修正、`version.txt` 更新までを一括で行う.
///
/// # Arguments
/// * `manifest_dir` - Cargo プロジェクトのルートディレクトリ.
/// * `vendor_dir` - `vendor/` ディレクトリのパス.
/// * `sysroot_dir` - 最終的な sysroot 配置先ディレクトリ.
/// * `latest_sha` - 今回適用する Raspberry Pi OS イメージの SHA256.
/// * `client` - イメージ取得に使う HTTP クライアント.
///
/// # Returns
/// sysroot の更新完了時に `Ok(())` を返す.
fn update_sysroot(
    manifest_dir: &Path,
    vendor_dir: &Path,
    sysroot_dir: &Path,
    latest_sha: &str,
    client: &Client,
) -> Result<()> {
    let cache_dir = vendor_dir.join(".rpi-sysroot-cache");
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("failed to create {}", cache_dir.display()))?;
    let temp_root_dir = cache_dir.join(".tmp");
    fs::create_dir_all(&temp_root_dir)
        .with_context(|| format!("failed to create {}", temp_root_dir.display()))?;
    cleanup_stale_temp_dirs(&temp_root_dir)?;

    let archive_path = cache_dir.join(format!("{latest_sha}.img.xz"));
    download_with_resume(client, IMAGE_URL, latest_sha, &archive_path)?;
    verify_archive_sha256(&archive_path, latest_sha)?;

    let work_dir = Builder::new()
        .prefix("work-")
        .tempdir_in(&temp_root_dir)
        .context("failed to create temporary working directory")?;
    let image_path = work_dir.path().join("raspios.img");
    let rootfs_image_path = work_dir.path().join("rootfs.ext4");
    extract_xz_image(&archive_path, &image_path)?;
    extract_root_partition_image(&image_path, &rootfs_image_path)?;

    let stage_dir = Builder::new()
        .prefix("stage-")
        .tempdir_in(&temp_root_dir)
        .context("failed to create sysroot staging directory")?;
    let staged_sysroot = stage_dir.path().join("rpi-sysroot");
    fs::create_dir_all(&staged_sysroot)
        .with_context(|| format!("failed to create {}", staged_sysroot.display()))?;

    extract_rootfs_subset_with_debugfs(&rootfs_image_path, &staged_sysroot)?;

    if has_absolute_symlinks(&staged_sysroot)? {
        run_sysroot_relativelinks_if_available(manifest_dir, &staged_sysroot)?;
        rewrite_absolute_symlinks(&staged_sysroot)?;
    }

    fs::write(
        staged_sysroot.join(VERSION_FILE_NAME),
        format!("{latest_sha}\n"),
    )
    .context("failed to write version.txt")?;

    replace_sysroot_directory(sysroot_dir, &staged_sysroot)?;
    drop(stage_dir);
    drop(work_dir);
    remove_dir_if_empty(&temp_root_dir)?;
    Ok(())
}

/// クロスリンクに必要な arm64 開発パッケージを sysroot に補完する.
fn ensure_cross_build_packages(
    vendor_dir: &Path,
    sysroot_dir: &Path,
    client: &Client,
) -> Result<()> {
    normalize_cross_build_sysroot(sysroot_dir)?;

    if cross_pkg_config_ready(sysroot_dir) {
        return Ok(());
    }

    let cache_dir = vendor_dir
        .join(".rpi-sysroot-cache")
        .join("debian-packages");
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("failed to create {}", cache_dir.display()))?;

    let suite =
        env::var("RPI_SYSROOT_DEBIAN_SUITE").unwrap_or_else(|_| DEFAULT_DEBIAN_SUITE.to_string());
    println!(
        "cargo:warning=Installing missing arm64 development packages into sysroot from Debian suite {suite}"
    );

    let packages_index = fetch_debian_packages_index(client, &cache_dir, &suite)?;
    let package_map = parse_debian_packages_index(&packages_index);

    for package_name in REQUIRED_SYSROOT_PACKAGES {
        let package = package_map.get(*package_name).ok_or_else(|| {
            anyhow!(
                "package {package_name} was not found in Debian suite {suite}; set RPI_SYSROOT_DEBIAN_SUITE if the image tracks a different release"
            )
        })?;

        let deb_path = cache_dir.join(
            Path::new(&package.filename)
                .file_name()
                .ok_or_else(|| anyhow!("invalid package path {}", package.filename))?,
        );
        let package_url = format!("{DEBIAN_MIRROR_URL}/{}", package.filename);

        download_file_with_sha256(client, &package_url, &deb_path, package.sha256.as_deref())?;
        run_command(
            Command::new("dpkg-deb")
                .arg("-x")
                .arg(&deb_path)
                .arg(sysroot_dir),
            "dpkg-deb -x",
        )
        .with_context(|| {
            format!(
                "failed to extract {} into {}",
                deb_path.display(),
                sysroot_dir.display()
            )
        })?;
    }

    normalize_cross_build_sysroot(sysroot_dir)?;

    if !cross_pkg_config_ready(sysroot_dir) {
        run_cross_pkg_config_probe(sysroot_dir).with_context(|| {
            format!(
                "sysroot still cannot satisfy pkg-config probe after package hydration: {}",
                sysroot_dir.display()
            )
        })?;
    }

    Ok(())
}

/// クロスビルド用 sysroot の symlink と lib 配置を正規化する.
fn normalize_cross_build_sysroot(sysroot_dir: &Path) -> Result<()> {
    if has_absolute_symlinks(sysroot_dir)? {
        rewrite_absolute_symlinks(sysroot_dir)?;
    }

    repair_broken_dev_symlinks(sysroot_dir)?;
    ensure_glibc_compat_symlinks(sysroot_dir)
}

/// pkg-config で最低限必要な依存解決ができる状態かを確認する.
fn cross_pkg_config_ready(sysroot_dir: &Path) -> bool {
    run_cross_pkg_config_probe(sysroot_dir).is_ok()
}

/// sysroot 向け pkg-config probe を実行する.
fn run_cross_pkg_config_probe(sysroot_dir: &Path) -> Result<()> {
    let mut command = cross_pkg_config_command(sysroot_dir);
    run_command(&mut command, "pkg-config cross probe")
}

/// sysroot 向け pkg-config 実行コマンドを組み立てる.
fn cross_pkg_config_command(sysroot_dir: &Path) -> Command {
    let mut command = Command::new("pkg-config");
    command.env("PKG_CONFIG_SYSROOT_DIR", sysroot_dir);
    command.env("PKG_CONFIG_LIBDIR", pkg_config_libdir_value(sysroot_dir));
    command.env("PKG_CONFIG_PATH", "");
    command.arg("--libs");
    command.arg("--cflags");
    command.args(CROSS_PKG_CONFIG_PROBE_MODULES);
    command
}

/// sysroot 内 pkg-config 検索パスを組み立てる.
fn pkg_config_libdir_value(sysroot_dir: &Path) -> String {
    [
        sysroot_dir.join("usr/lib/aarch64-linux-gnu/pkgconfig"),
        sysroot_dir.join("usr/lib/pkgconfig"),
        sysroot_dir.join("usr/share/pkgconfig"),
    ]
    .iter()
    .map(|path| path.display().to_string())
    .collect::<Vec<_>>()
    .join(":")
}

/// 壊れた開発用 `.so` symlink があれば、同ディレクトリ内の既存 SONAME へ張り直す.
fn repair_broken_dev_symlinks(sysroot_dir: &Path) -> Result<()> {
    for entry in WalkDir::new(sysroot_dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_symlink() {
            continue;
        }

        let link_path = entry.path();
        let file_name = entry.file_name().to_string_lossy();
        if !file_name.ends_with(".so") || link_path.exists() {
            continue;
        }

        let Some(parent) = link_path.parent() else {
            continue;
        };
        let prefix = format!("{file_name}.");
        let mut candidates = fs::read_dir(parent)
            .with_context(|| format!("failed to read {}", parent.display()))?
            .filter_map(|child| child.ok())
            .map(|child| child.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with(&prefix))
            .collect::<Vec<_>>();

        candidates.sort_by_key(|name| name.len());

        let Some(candidate_name) = candidates.into_iter().next() else {
            continue;
        };

        fs::remove_file(link_path)
            .with_context(|| format!("failed to remove broken symlink {}", link_path.display()))?;
        symlink(&candidate_name, link_path).with_context(|| {
            format!(
                "failed to rewrite broken symlink {} -> {}",
                link_path.display(),
                candidate_name
            )
        })?;
    }

    Ok(())
}

/// `/lib` が空に近い sysroot でも glibc ランタイム探索ができるよう互換 symlink を補う.
fn ensure_glibc_compat_symlinks(sysroot_dir: &Path) -> Result<()> {
    let lib_dir = sysroot_dir.join("lib");
    fs::create_dir_all(&lib_dir)
        .with_context(|| format!("failed to create {}", lib_dir.display()))?;

    let usr_multiarch_dir = sysroot_dir.join("usr/lib/aarch64-linux-gnu");
    if !usr_multiarch_dir.exists() {
        return Ok(());
    }

    let lib_multiarch_dir = lib_dir.join("aarch64-linux-gnu");
    if fs::symlink_metadata(&lib_multiarch_dir).is_err() {
        symlink("../usr/lib/aarch64-linux-gnu", &lib_multiarch_dir).with_context(|| {
            format!(
                "failed to create glibc compatibility symlink {}",
                lib_multiarch_dir.display()
            )
        })?;
    }

    let lib_loader = lib_dir.join("ld-linux-aarch64.so.1");
    if fs::symlink_metadata(&lib_loader).is_err() {
        symlink(
            "../usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1",
            &lib_loader,
        )
        .with_context(|| {
            format!(
                "failed to create dynamic loader compatibility symlink {}",
                lib_loader.display()
            )
        })?;
    }

    Ok(())
}

/// Debian Packages.xz を取得して展開した内容を返す.
fn fetch_debian_packages_index(client: &Client, cache_dir: &Path, suite: &str) -> Result<String> {
    let archive_path = cache_dir.join(format!("Packages-{suite}-main-arm64.xz"));
    let url = format!("{DEBIAN_MIRROR_URL}/dists/{suite}/main/binary-arm64/Packages.xz");

    download_file_with_sha256(client, &url, &archive_path, None)?;

    let file = File::open(&archive_path)
        .with_context(|| format!("failed to open {}", archive_path.display()))?;
    let mut decoder = XzDecoder::new(BufReader::new(file));
    let mut contents = String::new();
    decoder
        .read_to_string(&mut contents)
        .with_context(|| format!("failed to decompress {}", archive_path.display()))?;
    Ok(contents)
}

/// Debian Packages インデックスから package -> metadata の対応表を作る.
fn parse_debian_packages_index(contents: &str) -> HashMap<String, DebianPackage> {
    let mut packages = HashMap::new();

    for paragraph in contents.split("\n\n") {
        let mut package_name = None;
        let mut filename = None;
        let mut sha256 = None;

        for line in paragraph.lines() {
            if let Some(value) = line.strip_prefix("Package: ") {
                package_name = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("Filename: ") {
                filename = Some(value.trim().to_string());
            } else if let Some(value) = line.strip_prefix("SHA256: ") {
                sha256 = Some(value.trim().to_string());
            }
        }

        let (Some(package_name), Some(filename)) = (package_name, filename) else {
            continue;
        };

        packages.insert(package_name, DebianPackage { filename, sha256 });
    }

    packages
}

/// 任意の URL からファイルを取得し、必要なら SHA256 を検証する.
fn download_file_with_sha256(
    client: &Client,
    url: &str,
    destination_path: &Path,
    expected_sha256: Option<&str>,
) -> Result<()> {
    if destination_path.exists() {
        if let Some(expected_sha256) = expected_sha256 {
            if verify_archive_sha256(destination_path, expected_sha256).is_ok() {
                return Ok(());
            }
            fs::remove_file(destination_path).with_context(|| {
                format!("failed to remove stale file {}", destination_path.display())
            })?;
        } else {
            return Ok(());
        }
    }

    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("download endpoint returned an error for {url}"))?;

    let mut file = File::create(destination_path)
        .with_context(|| format!("failed to create {}", destination_path.display()))?;
    io::copy(&mut response, &mut file)
        .with_context(|| format!("failed while writing {}", destination_path.display()))?;
    file.flush()
        .with_context(|| format!("failed to flush {}", destination_path.display()))?;

    if let Some(expected_sha256) = expected_sha256 {
        verify_archive_sha256(destination_path, expected_sha256)?;
    }

    Ok(())
}

/// HTTP Range を使ってイメージを再開可能にダウンロードし、最後に SHA256 を検証する.
///
/// # Arguments
/// * `client` - ダウンロードに使う HTTP クライアント.
/// * `url` - ダウンロード対象 URL.
/// * `expected_sha` - 期待する SHA256.
/// * `archive_path` - キャッシュ済みアーカイブの保存先パス.
///
/// # Returns
/// ダウンロードと検証が完了したら `Ok(())` を返す.
fn download_with_resume(
    client: &Client,
    url: &str,
    expected_sha: &str,
    archive_path: &Path,
) -> Result<()> {
    let remote_size = fetch_remote_size(client, url).unwrap_or_default();
    if archive_path.exists() {
        let local_size = archive_path
            .metadata()
            .with_context(|| format!("failed to stat {}", archive_path.display()))?
            .len();

        if remote_size > 0 && local_size > remote_size {
            fs::remove_file(archive_path).with_context(|| {
                format!("failed to remove oversized file {}", archive_path.display())
            })?;
        }
    }

    if archive_path.exists() && verify_archive_sha256(archive_path, expected_sha).is_ok() {
        return Ok(());
    }

    for _attempt in 0..2 {
        let start = archive_path
            .metadata()
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        let mut request = client.get(url);
        if start > 0 {
            request = request.header(RANGE, format!("bytes={start}-"));
        }

        let mut response = request
            .send()
            .with_context(|| format!("failed to download image from {url}"))?;

        let status = response.status();
        if start > 0 && status == StatusCode::OK {
            fs::remove_file(archive_path).with_context(|| {
                format!(
                    "failed to reset partial download {}",
                    archive_path.display()
                )
            })?;
            continue;
        }

        if start > 0 && status != StatusCode::PARTIAL_CONTENT {
            bail!("server does not support HTTP range resume (status: {status})");
        }

        response = response
            .error_for_status()
            .with_context(|| format!("image endpoint returned an error for {url}"))?;

        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(start > 0)
            .write(true)
            .truncate(start == 0)
            .open(archive_path)
            .with_context(|| format!("failed to open {}", archive_path.display()))?;

        io::copy(&mut response, &mut file)
            .with_context(|| format!("failed while writing {}", archive_path.display()))?;
        file.flush()
            .with_context(|| format!("failed to flush {}", archive_path.display()))?;
        break;
    }

    verify_archive_sha256(archive_path, expected_sha)
}

/// HEAD リクエストからリモートファイルのサイズを取得する.
///
/// # Arguments
/// * `client` - 問い合わせに使う HTTP クライアント.
/// * `url` - サイズを取得する対象 URL.
///
/// # Returns
/// リモートファイルのバイト数を返す.
fn fetch_remote_size(client: &Client, url: &str) -> Result<u64> {
    let response = client
        .head(url)
        .send()
        .with_context(|| format!("failed to query content length for {url}"))?
        .error_for_status()
        .with_context(|| format!("HEAD request failed for {url}"))?;

    let length = response
        .headers()
        .get(CONTENT_LENGTH)
        .ok_or_else(|| anyhow!("missing Content-Length header for {url}"))?
        .to_str()
        .context("Content-Length header is not valid UTF-8")?
        .parse::<u64>()
        .context("failed to parse Content-Length header")?;

    Ok(length)
}

/// ファイルの SHA256 が期待値と一致することを確認する.
///
/// # Arguments
/// * `path` - 検証対象ファイルのパス.
/// * `expected_sha` - 期待する SHA256.
///
/// # Returns
/// 一致した場合に `Ok(())` を返す.
fn verify_archive_sha256(path: &Path, expected_sha: &str) -> Result<()> {
    let actual_sha = sha256_file(path)?;
    if actual_sha != expected_sha {
        bail!(
            "SHA256 mismatch for {}: expected {}, got {}",
            path.display(),
            expected_sha,
            actual_sha
        );
    }
    Ok(())
}

/// ファイル全体を読み取り、SHA256 を 16 進文字列で返す.
///
/// # Arguments
/// * `path` - ハッシュ化するファイルのパス.
///
/// # Returns
/// SHA256 の 16 進文字列を返す.
fn sha256_file(path: &Path) -> Result<String> {
    let mut file = BufReader::new(
        File::open(path).with_context(|| format!("failed to open {}", path.display()))?,
    );
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];

    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("failed while reading {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// `.img.xz` アーカイブを展開して `.img` を生成する.
///
/// # Arguments
/// * `archive_path` - 展開元の `.img.xz` アーカイブパス.
/// * `image_path` - 展開後 `.img` の出力先パス.
///
/// # Returns
/// 展開完了時に `Ok(())` を返す.
fn extract_xz_image(archive_path: &Path, image_path: &Path) -> Result<()> {
    let reader = BufReader::new(
        File::open(archive_path)
            .with_context(|| format!("failed to open {}", archive_path.display()))?,
    );
    let mut decoder = XzDecoder::new(reader);
    let mut writer = BufWriter::new(
        File::create(image_path)
            .with_context(|| format!("failed to create {}", image_path.display()))?,
    );

    io::copy(&mut decoder, &mut writer)
        .with_context(|| format!("failed to extract {}", archive_path.display()))?;
    writer
        .flush()
        .with_context(|| format!("failed to flush {}", image_path.display()))?;
    Ok(())
}

/// ディスクイメージから Linux rootfs パーティションを切り出す.
///
/// # Arguments
/// * `image_path` - 展開済み Raspberry Pi OS ディスクイメージのパス.
/// * `rootfs_image_path` - 切り出した ext4 rootfs イメージの出力先パス.
///
/// # Returns
/// rootfs パーティション抽出完了時に `Ok(())` を返す.
fn extract_root_partition_image(image_path: &Path, rootfs_image_path: &Path) -> Result<()> {
    let root_partition = find_root_partition(image_path)?;
    copy_partition_range(image_path, rootfs_image_path, &root_partition)
}

/// Raspberry Pi OS ディスクイメージから rootfs パーティション情報を取得する.
///
/// # Arguments
/// * `image_path` - 読み取り対象の Raspberry Pi OS ディスクイメージパス.
///
/// # Returns
/// 2 番パーティションの開始位置とサイズを返す.
fn find_root_partition(image_path: &Path) -> Result<PartitionRange> {
    let output = capture_command(
        Command::new("parted")
            .arg("-sm")
            .arg(image_path)
            .arg("unit")
            .arg("B")
            .arg("print"),
        "parted print",
    )
    .with_context(|| format!("failed to inspect partitions in {}", image_path.display()))?;

    for line in output.lines() {
        let line = line.trim();
        if !line.starts_with("2:") {
            continue;
        }

        let fields: Vec<_> = line.split(':').collect();
        if fields.len() < 4 {
            bail!("unexpected parted output for root partition: {line}");
        }

        return Ok(PartitionRange {
            start_bytes: parse_parted_bytes(fields[1])?,
            size_bytes: parse_parted_bytes(fields[3])?,
        });
    }

    bail!("failed to find partition 2 in {}", image_path.display())
}

/// `parted` のバイト表記を `u64` へ変換する.
///
/// # Arguments
/// * `value` - 末尾に `B` を持つバイト表記.
///
/// # Returns
/// バイト数を返す.
fn parse_parted_bytes(value: &str) -> Result<u64> {
    value
        .trim()
        .trim_end_matches('B')
        .parse::<u64>()
        .with_context(|| format!("failed to parse parted byte value: {value}"))
}

/// ディスクイメージ内の指定パーティション範囲を別ファイルへコピーする.
///
/// # Arguments
/// * `image_path` - 元のディスクイメージパス.
/// * `partition_image_path` - 切り出し先ファイルパス.
/// * `partition` - 切り出す開始位置とサイズ.
///
/// # Returns
/// パーティション切り出し完了時に `Ok(())` を返す.
fn copy_partition_range(
    image_path: &Path,
    partition_image_path: &Path,
    partition: &PartitionRange,
) -> Result<()> {
    if let Some(parent) = partition_image_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    run_command(
        Command::new("dd")
            .arg(format!("if={}", image_path.display()))
            .arg(format!("of={}", partition_image_path.display()))
            .arg("iflag=skip_bytes,count_bytes")
            .arg(format!("skip={}", partition.start_bytes))
            .arg(format!("count={}", partition.size_bytes)),
        "dd",
    )
    .with_context(|| {
        format!(
            "failed to extract partition range from {} to {}",
            image_path.display(),
            partition_image_path.display()
        )
    })
}

/// `debugfs` を使って rootfs から sysroot に必要なディレクトリだけを抽出する.
///
/// `/usr` と `/lib` は必須、`/opt` は存在する場合のみ抽出する.
/// Raspberry Pi OS 64bit では不要な `/opt/vc` は削除する.
///
/// # Arguments
/// * `rootfs_image_path` - 切り出した ext4 rootfs イメージのパス.
/// * `sysroot_dir` - 抽出先の sysroot ディレクトリ.
///
/// # Returns
/// 必要ディレクトリの抽出完了時に `Ok(())` を返す.
fn extract_rootfs_subset_with_debugfs(rootfs_image_path: &Path, sysroot_dir: &Path) -> Result<()> {
    debugfs_rdump(rootfs_image_path, "/usr", sysroot_dir, true)?;
    debugfs_rdump(rootfs_image_path, "/lib", sysroot_dir, true)?;

    if debugfs_path_exists(rootfs_image_path, "/opt")? {
        debugfs_rdump(rootfs_image_path, "/opt", sysroot_dir, false)?;
        remove_path_if_exists(&sysroot_dir.join("opt").join("vc"))?;
    }

    Ok(())
}

/// `debugfs stat` を使ってイメージ内パスの存在を確認する.
///
/// # Arguments
/// * `rootfs_image_path` - 切り出した ext4 rootfs イメージのパス.
/// * `path_in_image` - イメージ内で確認する絶対パス.
///
/// # Returns
/// 指定パスが存在すれば `true` を返す.
fn debugfs_path_exists(rootfs_image_path: &Path, path_in_image: &str) -> Result<bool> {
    let output = capture_command(
        Command::new("debugfs")
            .arg("-R")
            .arg(format!("stat {path_in_image}"))
            .arg(rootfs_image_path),
        "debugfs stat",
    )
    .with_context(|| {
        format!(
            "failed to inspect {} inside {}",
            path_in_image,
            rootfs_image_path.display()
        )
    })?;

    Ok(!debugfs_reports_missing_path(&output))
}

/// `debugfs rdump` を使ってイメージ内ディレクトリをホストへ展開する.
///
/// # Arguments
/// * `rootfs_image_path` - 切り出した ext4 rootfs イメージのパス.
/// * `path_in_image` - イメージ内で抽出する絶対パス.
/// * `destination_dir` - ホスト側の展開先ディレクトリ.
/// * `required` - `true` の場合は対象パス未存在をエラー扱いにする.
///
/// # Returns
/// 展開完了時に `Ok(())` を返す.
fn debugfs_rdump(
    rootfs_image_path: &Path,
    path_in_image: &str,
    destination_dir: &Path,
    required: bool,
) -> Result<()> {
    fs::create_dir_all(destination_dir)
        .with_context(|| format!("failed to create {}", destination_dir.display()))?;

    let output = capture_command(
        Command::new("debugfs")
            .arg("-R")
            .arg(format!(
                "rdump {path_in_image} {}",
                destination_dir.display()
            ))
            .arg(rootfs_image_path),
        "debugfs rdump",
    )
    .with_context(|| {
        format!(
            "failed to extract {} from {}",
            path_in_image,
            rootfs_image_path.display()
        )
    })?;

    if debugfs_reports_missing_path(&output) {
        if required {
            bail!(
                "required path {} was not found in {}",
                path_in_image,
                rootfs_image_path.display()
            );
        }
        return Ok(());
    }

    Ok(())
}

/// `debugfs` 出力がパス未存在を示しているか判定する.
///
/// # Arguments
/// * `output` - `debugfs` の標準出力.
///
/// # Returns
/// 未存在メッセージを含む場合に `true` を返す.
fn debugfs_reports_missing_path(output: &str) -> bool {
    output.contains("File not found by ext2_lookup") || output.contains("File not found")
}

/// ディスクイメージ内パーティションのバイト範囲.
struct PartitionRange {
    start_bytes: u64,
    size_bytes: u64,
}

/// `.tmp` 配下に残った古い作業ディレクトリを削除する.
///
/// # Arguments
/// * `temp_root_dir` - 一時ディレクトリ群を保持するルートディレクトリ.
///
/// # Returns
/// 掃除完了時に `Ok(())` を返す.
fn cleanup_stale_temp_dirs(temp_root_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(temp_root_dir)
        .with_context(|| format!("failed to read {}", temp_root_dir.display()))?
    {
        let entry = entry?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if !file_type.is_dir() {
            continue;
        }

        let file_name = entry.file_name();
        if file_name != OsStr::new("work")
            && file_name != OsStr::new("stage")
            && !file_name.to_string_lossy().starts_with("work-")
            && !file_name.to_string_lossy().starts_with("stage-")
        {
            continue;
        }

        fs::remove_dir_all(entry.path()).with_context(|| {
            format!(
                "failed to remove stale temp directory {}",
                entry.path().display()
            )
        })?;
    }

    Ok(())
}

/// ディレクトリが空なら削除する.
///
/// # Arguments
/// * `path` - 空なら削除するディレクトリパス.
///
/// # Returns
/// 削除済みまたは未存在、または空でない場合に `Ok(())` を返す.
fn remove_dir_if_empty(path: &Path) -> Result<()> {
    match fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().is_none() {
                fs::remove_dir(path).with_context(|| {
                    format!("failed to remove empty directory {}", path.display())
                })?;
            }
            Ok(())
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

/// ファイルまたはディレクトリが存在すれば削除する.
///
/// 壊れた symlink に対しても `symlink_metadata` を用いて安全に扱う.
///
/// # Arguments
/// * `path` - 削除対象のパス.
///
/// # Returns
/// 削除済みまたは未存在なら `Ok(())` を返す.
fn remove_path_if_exists(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("failed to stat {}", path.display())),
    };

    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

/// sysroot 内に絶対パス symlink が残っているかどうかを確認する.
///
/// # Arguments
/// * `sysroot_dir` - 走査対象の sysroot ディレクトリ.
///
/// # Returns
/// 絶対 symlink が 1 つでも見つかれば `true` を返す.
fn has_absolute_symlinks(sysroot_dir: &Path) -> Result<bool> {
    for entry in WalkDir::new(sysroot_dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_symlink() {
            continue;
        }

        let target = fs::read_link(entry.path())?;
        if target.is_absolute() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `sysroot-relativelinks.py` が見つかった場合のみ実行する.
///
/// # Arguments
/// * `manifest_dir` - Cargo プロジェクトのルートディレクトリ.
/// * `sysroot_dir` - 相対リンク化の対象 sysroot ディレクトリ.
///
/// # Returns
/// スクリプト未検出または正常終了時に `Ok(())` を返す.
fn run_sysroot_relativelinks_if_available(manifest_dir: &Path, sysroot_dir: &Path) -> Result<()> {
    let script_path = find_sysroot_relativelinks_script(manifest_dir);
    if let Some(script_path) = script_path {
        run_command(
            Command::new("python3").arg(&script_path).arg(sysroot_dir),
            "python3 sysroot-relativelinks.py",
        )
        .with_context(|| format!("failed to run {}", script_path.display()))?;
    }
    Ok(())
}

/// `vendor/` または `vender/` 配下から `sysroot-relativelinks.py` を探索する.
///
/// # Arguments
/// * `manifest_dir` - Cargo プロジェクトのルートディレクトリ.
///
/// # Returns
/// スクリプトが見つかればそのパスを返す.
fn find_sysroot_relativelinks_script(manifest_dir: &Path) -> Option<PathBuf> {
    let candidates = [manifest_dir.join("vendor"), manifest_dir.join("vender")];
    for base in candidates {
        if !base.exists() {
            continue;
        }

        for entry in WalkDir::new(base).follow_links(false) {
            let Ok(entry) = entry else {
                continue;
            };
            if entry.file_name() == OsStr::new("sysroot-relativelinks.py") {
                return Some(entry.into_path());
            }
        }
    }
    None
}

/// sysroot 内の絶対 symlink を相対 symlink へ書き換える.
///
/// # Arguments
/// * `sysroot_dir` - 書き換え対象の sysroot ディレクトリ.
///
/// # Returns
/// 書き換え完了時に `Ok(())` を返す.
fn rewrite_absolute_symlinks(sysroot_dir: &Path) -> Result<()> {
    for entry in WalkDir::new(sysroot_dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_symlink() {
            continue;
        }

        let link_path = entry.path();
        let target = fs::read_link(link_path)
            .with_context(|| format!("failed to read symlink {}", link_path.display()))?;

        if !target.is_absolute() {
            continue;
        }

        let relative_target = target
            .strip_prefix("/")
            .with_context(|| format!("failed to strip leading slash from {}", target.display()))?;
        let absolute_target = sysroot_dir.join(relative_target);
        let parent = link_path
            .parent()
            .ok_or_else(|| anyhow!("{} has no parent directory", link_path.display()))?;

        let new_target = diff_paths(&absolute_target, parent).unwrap_or(absolute_target);
        fs::remove_file(link_path)
            .with_context(|| format!("failed to remove {}", link_path.display()))?;
        symlink(&new_target, link_path)
            .with_context(|| format!("failed to rewrite symlink {}", link_path.display()))?;
    }
    Ok(())
}

/// ステージング済み sysroot を最終配置先へアトミックに近い形で置き換える.
///
/// # Arguments
/// * `destination` - 最終配置先の sysroot ディレクトリ.
/// * `staged_sysroot` - ステージング済み sysroot ディレクトリ.
///
/// # Returns
/// 置き換え完了時に `Ok(())` を返す.
fn replace_sysroot_directory(destination: &Path, staged_sysroot: &Path) -> Result<()> {
    if destination.exists() {
        fs::remove_dir_all(destination)
            .with_context(|| format!("failed to remove {}", destination.display()))?;
    }

    fs::rename(staged_sysroot, destination).with_context(|| {
        format!(
            "failed to move staged sysroot {} to {}",
            staged_sysroot.display(),
            destination.display()
        )
    })?;
    Ok(())
}

/// 外部コマンドを実行し、失敗時は標準出力・標準エラー付きでエラー化する.
///
/// # Arguments
/// * `command` - 実行するコマンド.
/// * `label` - エラーメッセージに使う識別名.
///
/// # Returns
/// コマンド成功時に `Ok(())` を返す.
fn run_command(command: &mut Command, label: &str) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("failed to start {label}"))?;
    ensure_success(&output, label)
}

/// 外部コマンドを実行し、成功時の標準出力を文字列として返す.
///
/// # Arguments
/// * `command` - 実行するコマンド.
/// * `label` - エラーメッセージに使う識別名.
///
/// # Returns
/// 標準出力を trim 済み文字列として返す.
fn capture_command(command: &mut Command, label: &str) -> Result<String> {
    let output = command
        .output()
        .with_context(|| format!("failed to start {label}"))?;
    ensure_success(&output, label)
        .map(|()| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// コマンド終了コードを確認し、失敗時は詳細な診断情報を返す.
///
/// # Arguments
/// * `output` - 実行済みコマンドの出力結果.
/// * `label` - エラーメッセージに使う識別名.
///
/// # Returns
/// 終了ステータスが成功なら `Ok(())` を返す.
fn ensure_success(output: &Output, label: &str) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!(
        "{label} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        stdout.trim(),
        stderr.trim()
    );
}
