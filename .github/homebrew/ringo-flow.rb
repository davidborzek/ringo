# Template — placeholders (__VERSION__, __BASE__, __SHA_*__) are filled in by the
# `homebrew` job in release-plz.yml and the result is pushed to the tap repo.
class RingoFlow < Formula
  desc "Declarative telephony scenario test runner for baresip"
  homepage "https://github.com/davidborzek/ringo"
  version "__VERSION__"
  license "MIT"

  depends_on "spandsp"
  depends_on "opus"

  on_macos do
    on_arm do
      url "__BASE__/ringo-flow-__VERSION__-aarch64-apple-darwin.tar.gz"
      sha256 "__SHA_DARWIN_ARM__"
    end
    on_intel do
      url "__BASE__/ringo-flow-__VERSION__-x86_64-apple-darwin.tar.gz"
      sha256 "__SHA_DARWIN_X64__"
    end
  end

  on_linux do
    on_arm do
      url "__BASE__/ringo-flow-__VERSION__-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "__SHA_LINUX_ARM__"
    end
    on_intel do
      url "__BASE__/ringo-flow-__VERSION__-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "__SHA_LINUX_X64__"
    end
  end

  def install
    bin.install "ringo-flow"
  end

  test do
    system bin/"ringo-flow", "--help"
  end
end
