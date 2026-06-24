# Template — placeholders (__VERSION__, __BASE__, __SHA_*__) are filled in by the
# `homebrew` job in release-plz.yml and the result is pushed to the tap repo.
class Ringo < Formula
  desc "A terminal SIP softphone built on baresip"
  homepage "https://github.com/davidborzek/ringo"
  version "__VERSION__"
  license "MIT"

  on_macos do
    on_arm do
      url "__BASE__/ringo-aarch64-apple-darwin"
      sha256 "__SHA_DARWIN_ARM__"
    end
    on_intel do
      url "__BASE__/ringo-x86_64-apple-darwin"
      sha256 "__SHA_DARWIN_X64__"
    end
  end

  on_linux do
    on_arm do
      url "__BASE__/ringo-aarch64-unknown-linux-gnu"
      sha256 "__SHA_LINUX_ARM__"
    end
    on_intel do
      url "__BASE__/ringo-x86_64-unknown-linux-gnu"
      sha256 "__SHA_LINUX_X64__"
    end
  end

  def install
    bin.install Dir["ringo-*"].first => "ringo"
  end

  test do
    system bin/"ringo", "--help"
  end
end
