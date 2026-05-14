#!/usr/bin/env sh
set -eu

repo="ianzepp/orqa"
bin_name="orqa"
install_dir="${ORQA_INSTALL_DIR:-$HOME/.local/bin}"

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "orqa installer: missing required command: $1" >&2
    exit 1
  }
}

need curl
need tar
need uname

os="$(uname -s)"
arch="$(uname -m)"

case "$os:$arch" in
  Darwin:arm64) target="aarch64-apple-darwin" ;;
  Darwin:x86_64) target="x86_64-apple-darwin" ;;
  Linux:x86_64) target="x86_64-unknown-linux-gnu" ;;
  *)
    echo "orqa installer: unsupported platform: $os $arch" >&2
    exit 1
    ;;
esac

base_url="https://github.com/$repo/releases/latest/download"
archive="$bin_name-$target.tar.gz"
tmp_dir="$(mktemp -d)"

cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

curl -fsSL "$base_url/$archive" -o "$tmp_dir/$archive"
curl -fsSL "$base_url/$archive.sha256" -o "$tmp_dir/$archive.sha256"

if command -v shasum >/dev/null 2>&1; then
  (cd "$tmp_dir" && shasum -a 256 -c "$archive.sha256")
elif command -v sha256sum >/dev/null 2>&1; then
  (cd "$tmp_dir" && sha256sum -c "$archive.sha256")
else
  echo "orqa installer: warning: shasum or sha256sum not found; skipping checksum verification" >&2
fi

mkdir -p "$install_dir"
tar -C "$tmp_dir" -xzf "$tmp_dir/$archive"
install "$tmp_dir/$bin_name" "$install_dir/$bin_name"

echo "installed $bin_name to $install_dir/$bin_name"
