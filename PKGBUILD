#!/usr/bin/env bash
# Maintainer: Your Name <your.email@example.com>
# shell: bash
# shellcheck disable=SC2034,SC2154
pkgname=ssgg
pkgver=0.1.2 # x-release-please-version
pkgrel=1
pkgdesc="A complete open-source SteelSeries GG replacement for Linux - RGB lighting, audio mixer, and GameSense support"
arch=('x86_64')
url="https://github.com/Ven0m0/steelseriesgg-rs"
license=('MIT')
depends=('hidapi' 'glibc' 'systemd')
makedepends=('rust')
install=ssgg.install
source=(
  "$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz"
)
# Update this when creating a release
sha256sums=('SKIP')

_archive_dir="steelseriesgg-rs-$pkgver"
_rust_target="$CARCH-unknown-linux-gnu"

# NOTE: $srcdir is only populated when makepkg invokes the build functions, not
# while the PKGBUILD is sourced. Deriving CARGO_HOME/CARGO_TARGET_DIR at parse
# time would expand to /cargo-home and /target, which the unprivileged build
# user cannot write to. Reference $srcdir inside the functions instead.

_resolve_archive_dir() {
  local candidate
  for candidate in \
    "$srcdir/$_archive_dir" \
    "$srcdir/$pkgname-$pkgver" \
    "$srcdir/${url##*/}-$pkgver"
  do
    if [[ -d "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  printf 'Missing extracted source directory in %s\n' "$srcdir" >&2
  return 1
}

prepare() {
  cd "$(_resolve_archive_dir)" || return 1
  export CARGO_HOME="$srcdir/cargo-home"
  cargo fetch --locked --target "$_rust_target"
}

build() {
  cd "$(_resolve_archive_dir)" || return 1
  export CARGO_HOME="$srcdir/cargo-home"
  export CARGO_TARGET_DIR="$srcdir/target"
  cargo build --release --frozen --bin ssgg
}

check() {
  cd "$(_resolve_archive_dir)" || return 1
  export CARGO_HOME="$srcdir/cargo-home"
  export CARGO_TARGET_DIR="$srcdir/target"
  cargo test --frozen
}

package() {
  cd "$(_resolve_archive_dir)" || return 1
  install -Dm755 "$srcdir/target/release/ssgg" "$pkgdir/usr/bin/ssgg"
  install -Dm644 assets/ssgg.service "$pkgdir/usr/lib/systemd/user/ssgg.service"
  install -Dm644 assets/99-steelseries.rules "$pkgdir/usr/lib/udev/rules.d/99-steelseries.rules"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
