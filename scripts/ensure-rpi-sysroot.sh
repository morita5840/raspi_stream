#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_dir=$(CDPATH= cd -- "$script_dir/.." && pwd)
sysroot="$repo_dir/vendor/rpi-sysroot"
version_file="$sysroot/version.txt"
cache_dir="$repo_dir/vendor/.rpi-sysroot-cache"
stamp_file="$cache_dir/last-check-aarch64"
lock_dir="$cache_dir/ensure.lock"
check_interval="${RPI_SYSROOT_CHECK_INTERVAL_SECONDS:-3600}"
image_url="https://downloads.raspberrypi.com/raspios_lite_arm64_latest"
sha256_url="https://downloads.raspberrypi.com/raspios_lite_arm64_latest.sha256"
debian_mirror_url="https://deb.debian.org/debian"
debian_suite="${RPI_SYSROOT_DEBIAN_SUITE:-bookworm}"
required_sysroot_packages='libblkid-dev libdw-dev libelf-dev libffi-dev libglib2.0-dev liblzma-dev libmount-dev liborc-0.4-0 liborc-0.4-dev libselinux1-dev libsepol-dev libunwind8 libunwind-dev libpcre2-dev libgstreamer1.0-0 libgstreamer1.0-dev libgstreamer-plugins-base1.0-0 libgstreamer-plugins-base1.0-dev libgstrtspserver-1.0-0 libgstrtspserver-1.0-dev zlib1g-dev'

log() {
    printf '%s\n' "$*" >&2
}

die() {
    log "$*"
    exit 1
}

env_true() {
    case "${1:-}" in
        1|true|TRUE) return 0 ;;
        *) return 1 ;;
    esac
}

verify_sha256() {
    file_path=$1
    expected_sha=$2
    printf '%s  %s\n' "$expected_sha" "$file_path" | sha256sum -c - --status
}

fetch_latest_sha256() {
    curl -fsSL "$sha256_url" | awk 'NR == 1 { print tolower($1); exit }'
}

if [ "${1:-}" = "--print-latest-sha256" ]; then
    fetch_latest_sha256
    exit 0
fi

fetch_remote_size() {
    curl -fsSLI "$1" | awk -F': *' 'tolower($1) == "content-length" { gsub(/\r/, "", $2); size = $2 } END { if (size != "") print size }'
}

download_with_resume() {
    url=$1
    expected_sha=$2
    destination_path=$3

    remote_size=$(fetch_remote_size "$url" || true)
    if [ -f "$destination_path" ] && [ -n "$remote_size" ]; then
        local_size=$(wc -c < "$destination_path")
        if [ "$local_size" -gt "$remote_size" ]; then
            rm -f "$destination_path"
        fi
    fi

    if [ -f "$destination_path" ] && verify_sha256 "$destination_path" "$expected_sha"; then
        return 0
    fi

    mkdir -p "$(dirname "$destination_path")"
    if ! curl -fsSL -C - -o "$destination_path" "$url"; then
        rm -f "$destination_path"
        curl -fsSL -o "$destination_path" "$url"
    fi

    verify_sha256 "$destination_path" "$expected_sha" || die "SHA256 mismatch for $destination_path"
}

download_file() {
    url=$1
    destination_path=$2
    expected_sha=${3:-}

    if [ -f "$destination_path" ]; then
        if [ -n "$expected_sha" ] && ! verify_sha256 "$destination_path" "$expected_sha"; then
            rm -f "$destination_path"
        else
            return 0
        fi
    fi

    mkdir -p "$(dirname "$destination_path")"
    curl -fsSL -o "$destination_path" "$url"

    if [ -n "$expected_sha" ]; then
        verify_sha256 "$destination_path" "$expected_sha" || die "SHA256 mismatch for $destination_path"
    fi
}

cleanup_stale_temp_dirs() {
    temp_root_dir=$1
    [ -d "$temp_root_dir" ] || return 0
    find "$temp_root_dir" -mindepth 1 -maxdepth 1 -type d \( -name 'work' -o -name 'stage' -o -name 'work-*' -o -name 'stage-*' \) -exec rm -rf {} +
}

run_cross_pkg_config_probe() {
    PKG_CONFIG_SYSROOT_DIR="$sysroot" \
    PKG_CONFIG_LIBDIR="$sysroot/usr/lib/aarch64-linux-gnu/pkgconfig:$sysroot/usr/lib/pkgconfig:$sysroot/usr/share/pkgconfig" \
    PKG_CONFIG_PATH= \
        pkg-config --libs --cflags glib-2.0 gstreamer-rtsp-server-1.0 >/dev/null 2>&1
}

has_absolute_symlinks() {
    python3 - "$1" <<'PY'
import os
import sys

sysroot = sys.argv[1]
for root, dirs, files in os.walk(sysroot, followlinks=False):
    for name in dirs + files:
        path = os.path.join(root, name)
        if not os.path.islink(path):
            continue
        if os.readlink(path).startswith('/'):
            sys.exit(0)
sys.exit(1)
PY
}

rewrite_absolute_symlinks() {
    python3 - "$1" <<'PY'
import os
import sys

sysroot = os.path.abspath(sys.argv[1])
for root, dirs, files in os.walk(sysroot, followlinks=False):
    for name in dirs + files:
        path = os.path.join(root, name)
        if not os.path.islink(path):
            continue
        target = os.readlink(path)
        if not os.path.isabs(target):
            continue
        abs_target = os.path.join(sysroot, target.lstrip('/'))
        rel_target = os.path.relpath(abs_target, os.path.dirname(path))
        os.remove(path)
        os.symlink(rel_target, path)
PY
}

repair_broken_dev_symlinks() {
    python3 - "$1" <<'PY'
import os
import sys

sysroot = sys.argv[1]
for root, dirs, files in os.walk(sysroot, followlinks=False):
    for name in dirs + files:
        path = os.path.join(root, name)
        if not os.path.islink(path):
            continue
        base = os.path.basename(path)
        if not base.endswith('.so') or os.path.exists(path):
            continue
        prefix = base + '.'
        candidates = sorted(
            child for child in os.listdir(root)
            if child.startswith(prefix)
        )
        if not candidates:
            continue
        os.remove(path)
        os.symlink(candidates[0], path)
PY
}

ensure_glibc_compat_symlinks() {
    mkdir -p "$sysroot/lib"
    if [ ! -d "$sysroot/usr/lib/aarch64-linux-gnu" ]; then
        return 0
    fi

    if [ ! -e "$sysroot/lib/aarch64-linux-gnu" ]; then
        ln -s ../usr/lib/aarch64-linux-gnu "$sysroot/lib/aarch64-linux-gnu"
    fi

    if [ ! -e "$sysroot/lib/ld-linux-aarch64.so.1" ]; then
        ln -s ../usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1 "$sysroot/lib/ld-linux-aarch64.so.1"
    fi
}

normalize_cross_build_sysroot() {
    if has_absolute_symlinks "$sysroot"; then
        rewrite_absolute_symlinks "$sysroot"
    fi
    repair_broken_dev_symlinks "$sysroot"
    ensure_glibc_compat_symlinks
}

find_sysroot_relativelinks_script() {
    for candidate_root in "$repo_dir/vendor" "$repo_dir/vender"; do
        [ -d "$candidate_root" ] || continue
        found=$(find "$candidate_root" -name sysroot-relativelinks.py -print -quit 2>/dev/null || true)
        if [ -n "$found" ]; then
            printf '%s\n' "$found"
            return 0
        fi
    done
    return 1
}

run_sysroot_relativelinks_if_available() {
    if script_path=$(find_sysroot_relativelinks_script); then
        python3 "$script_path" "$1"
    fi
}

debugfs_path_exists() {
    output=$(debugfs -R "stat $2" "$1" 2>&1 || true)
    case "$output" in
        *"File not found by ext2_lookup"*|*"File not found"*) return 1 ;;
        *) return 0 ;;
    esac
}

debugfs_rdump() {
    rootfs_image_path=$1
    path_in_image=$2
    destination_dir=$3
    required=$4

    mkdir -p "$destination_dir"
    output=$(debugfs -R "rdump $path_in_image $destination_dir" "$rootfs_image_path" 2>&1 || true)
    case "$output" in
        *"File not found by ext2_lookup"*|*"File not found"*)
            [ "$required" = 1 ] && die "required path $path_in_image was not found in $rootfs_image_path"
            ;;
    esac
}

extract_rootfs_subset_with_debugfs() {
    rootfs_image_path=$1
    destination_dir=$2

    debugfs_rdump "$rootfs_image_path" /usr "$destination_dir" 1
    debugfs_rdump "$rootfs_image_path" /lib "$destination_dir" 1

    if debugfs_path_exists "$rootfs_image_path" /opt; then
        debugfs_rdump "$rootfs_image_path" /opt "$destination_dir" 0
        rm -rf "$destination_dir/opt/vc"
    fi
}

extract_root_partition_image() {
    image_path=$1
    rootfs_image_path=$2
    partition_info=$(parted -sm "$image_path" unit B print | awk -F: '$1 == "2" { gsub(/B$/, "", $2); gsub(/B$/, "", $4); print $2 " " $4; exit }')
    [ -n "$partition_info" ] || die "failed to find partition 2 in $image_path"
    start_bytes=$(printf '%s\n' "$partition_info" | awk '{ print $1 }')
    size_bytes=$(printf '%s\n' "$partition_info" | awk '{ print $2 }')
    dd if="$image_path" of="$rootfs_image_path" iflag=skip_bytes,count_bytes skip="$start_bytes" count="$size_bytes" status=none
}

download_debian_packages_index() {
    archive_path=$1
    url="$debian_mirror_url/dists/$debian_suite/main/binary-arm64/Packages.xz"
    download_file "$url" "$archive_path"
}

install_missing_sysroot_packages() {
    normalize_cross_build_sysroot
    if run_cross_pkg_config_probe; then
        return 0
    fi

    packages_archive="$cache_dir/debian-packages/Packages-$debian_suite-main-arm64.xz"
    mkdir -p "$cache_dir/debian-packages"
    download_debian_packages_index "$packages_archive"

    package_map=$(python3 - "$packages_archive" $required_sysroot_packages <<'PY'
import lzma
import sys

archive = sys.argv[1]
required = list(sys.argv[2:])
required_set = set(required)
found = {}

with lzma.open(archive, 'rt', encoding='utf-8') as handle:
    package = filename = sha256 = None
    for line in handle:
        line = line.rstrip('\n')
        if not line:
            if package in required_set and filename:
                found[package] = (filename, sha256 or '')
            package = filename = sha256 = None
            continue
        if line.startswith('Package: '):
            package = line[len('Package: '):].strip()
        elif line.startswith('Filename: '):
            filename = line[len('Filename: '):].strip()
        elif line.startswith('SHA256: '):
            sha256 = line[len('SHA256: '):].strip()
    if package in required_set and filename:
        found[package] = (filename, sha256 or '')

missing = [name for name in required if name not in found]
if missing:
    print('missing:' + ','.join(missing), file=sys.stderr)
    sys.exit(1)

for name in required:
    filename, sha256 = found[name]
    print(f'{name}\t{filename}\t{sha256}')
PY
)

    tab=$(printf '\t')
    printf '%s\n' "$package_map" | while IFS="$tab" read -r package_name package_filename package_sha; do
        [ -n "$package_name" ] || continue
        deb_path="$cache_dir/debian-packages/$(basename "$package_filename")"
        package_url="$debian_mirror_url/$package_filename"
        download_file "$package_url" "$deb_path" "$package_sha"
        dpkg-deb -x "$deb_path" "$sysroot"
    done

    normalize_cross_build_sysroot
    run_cross_pkg_config_probe || die "sysroot still cannot satisfy pkg-config probe after package hydration: $sysroot"
}

prepare_sysroot() {
    [ -f /proc/device-tree/model ] && grep -q 'Raspberry Pi' /proc/device-tree/model && return 0
    env_true "${RPI_SYSROOT_SKIP:-}" && return 0

    latest_sha=$(fetch_latest_sha256)
    current_sha=$(cat "$version_file" 2>/dev/null | tr -d '[:space:]' || true)

    if [ "$current_sha" = "$latest_sha" ] \
        && [ -d "$sysroot/usr" ] \
        && [ -d "$sysroot/lib" ] \
        && ! env_true "${RPI_SYSROOT_FORCE_CHECK:-}"; then
        install_missing_sysroot_packages
        return 0
    fi

    mkdir -p "$cache_dir" "$cache_dir/.tmp"
    cleanup_stale_temp_dirs "$cache_dir/.tmp"

    archive_path="$cache_dir/$latest_sha.img.xz"
    download_with_resume "$image_url" "$latest_sha" "$archive_path"

    work_dir=$(mktemp -d "$cache_dir/.tmp/work-XXXXXX")
    stage_dir=$(mktemp -d "$cache_dir/.tmp/stage-XXXXXX")
    image_path="$work_dir/raspios.img"
    rootfs_image_path="$work_dir/rootfs.ext4"
    staged_sysroot="$stage_dir/rpi-sysroot"
    mkdir -p "$staged_sysroot"

    xz -dc "$archive_path" > "$image_path"
    extract_root_partition_image "$image_path" "$rootfs_image_path"
    extract_rootfs_subset_with_debugfs "$rootfs_image_path" "$staged_sysroot"

    if has_absolute_symlinks "$staged_sysroot"; then
        run_sysroot_relativelinks_if_available "$staged_sysroot"
        rewrite_absolute_symlinks "$staged_sysroot"
    fi

    printf '%s\n' "$latest_sha" > "$staged_sysroot/version.txt"
    rm -rf "$sysroot"
    mv "$staged_sysroot" "$sysroot"
    rm -rf "$stage_dir" "$work_dir"
    rmdir "$cache_dir/.tmp" 2>/dev/null || true

    install_missing_sysroot_packages
}

needs_prepare() {
    [ -f "$version_file" ] || return 0

    for pc_file in \
        "$sysroot/usr/lib/aarch64-linux-gnu/pkgconfig/glib-2.0.pc" \
        "$sysroot/usr/lib/aarch64-linux-gnu/pkgconfig/gstreamer-rtsp-server-1.0.pc"
    do
        [ -f "$pc_file" ] || return 0
    done

    return 1
}

stamp_is_fresh() {
    [ -f "$stamp_file" ] || return 1
    [ "$check_interval" -gt 0 ] || return 1

    now=$(date +%s)
    last=$(stat -c %Y "$stamp_file" 2>/dev/null || echo 0)
    age=$((now - last))
    [ "$age" -lt "$check_interval" ]
}

mkdir -p "$cache_dir"

if ! needs_prepare && stamp_is_fresh && [ -z "${RPI_SYSROOT_FORCE_CHECK:-}" ]; then
    exit 0
fi

while ! mkdir "$lock_dir" 2>/dev/null; do
    sleep 1
done
trap 'rmdir "$lock_dir"' EXIT INT TERM HUP

if needs_prepare || [ -n "${RPI_SYSROOT_FORCE_CHECK:-}" ] || ! stamp_is_fresh; then
    prepare_sysroot
fi

touch "$stamp_file"