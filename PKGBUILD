#!/usr/bin/env bash
# Maintainer: Your Name <your.email@example.com>
# shell: bash
# shellcheck disable=SC2034,SC2154
pkgname=ssgg
pkgver=0.1.0
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
_cargo_home="$srcdir/cargo-home"
_cargo_target_dir="$srcdir/target"
_rust_target="$CARCH-unknown-linux-gnu"

prepare() {
  cd "$_archive_dir" || return 1
  export CARGO_HOME="$_cargo_home"
  cargo fetch --locked --target "$_rust_target"
}

build() {
  cd "$_archive_dir" || return 1
  export CARGO_HOME="$_cargo_home"
  export CARGO_TARGET_DIR="$_cargo_target_dir"
  cargo build --release --frozen --bin ssgg
}

check() {
  cd "$_archive_dir" || return 1
  export CARGO_HOME="$_cargo_home"
  export CARGO_TARGET_DIR="$_cargo_target_dir"
  cargo test --frozen
}

package() {
  cd "$_archive_dir" || return 1
  install -Dm755 "$_cargo_target_dir/release/ssgg" "$pkgdir/usr/bin/ssgg"
  install -Dm644 assets/ssgg.service "$pkgdir/usr/lib/systemd/user/ssgg.service"
  install -Dm644 assets/99-steelseries.rules "$pkgdir/usr/lib/udev/rules.d/99-steelseries.rules"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
