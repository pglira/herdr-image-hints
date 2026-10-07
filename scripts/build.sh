#!/bin/sh
# Puts the plugin binary at target/release/herdr-image-hints: built with cargo
# when cargo is on PATH, else downloaded as the static binary of this version
# from the GitHub release.
set -eu
cd "$(dirname "$0")/.."

if command -v cargo >/dev/null 2>&1; then
    exec cargo build --release --locked
fi

version="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) asset="herdr-image-hints-x86_64-unknown-linux-musl" ;;
    *)
        echo "herdr-image-hints: no prebuilt binary for $(uname -s)-$(uname -m);" \
            "install Rust (https://rustup.rs) and install the plugin again" >&2
        exit 1
        ;;
esac
url="https://github.com/pglira/herdr-image-hints/releases/download/v${version}/${asset}"

mkdir -p target/release
part="target/release/herdr-image-hints.part"
if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$part" "$url"
elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$part" "$url"
else
    echo "herdr-image-hints: neither cargo nor curl/wget is available" >&2
    exit 1
fi
chmod +x "$part"
mv "$part" target/release/herdr-image-hints
