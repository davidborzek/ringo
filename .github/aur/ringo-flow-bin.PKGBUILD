# Maintainer: David Borzek <mail@davidborzek.de>
# Template — placeholders (__VERSION__, __BASE__, __SHA_*__) are filled in by the
# `aur` job in release-plz.yml and the result is pushed to the AUR repo as PKGBUILD.
pkgname=ringo-flow-bin
pkgver=__VERSION__
pkgrel=1
pkgdesc="Declarative telephony scenario test runner for baresip"
arch=('x86_64' 'aarch64')
url="https://github.com/davidborzek/ringo"
license=('MIT')
# baresip/libre/OpenSSL are statically linked into the binary; opus and spandsp
# are linked dynamically (same as the Homebrew formula).
depends=('opus' 'spandsp')
provides=('ringo-flow')
conflicts=('ringo-flow')
source_x86_64=("ringo-flow-$pkgver-x86_64.tar.gz::__BASE__/ringo-flow-__VERSION__-x86_64-unknown-linux-gnu.tar.gz")
source_aarch64=("ringo-flow-$pkgver-aarch64.tar.gz::__BASE__/ringo-flow-__VERSION__-aarch64-unknown-linux-gnu.tar.gz")
sha256sums_x86_64=('__SHA_LINUX_X64__')
sha256sums_aarch64=('__SHA_LINUX_ARM__')

package() {
  case "$CARCH" in
    x86_64) _target="x86_64-unknown-linux-gnu" ;;
    aarch64) _target="aarch64-unknown-linux-gnu" ;;
  esac
  install -Dm755 "$srcdir/ringo-flow-$pkgver-$_target/ringo-flow" "$pkgdir/usr/bin/ringo-flow"
}
