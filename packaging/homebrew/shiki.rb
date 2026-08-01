class Shiki < Formula
  desc "TUI note-taking app with a Yazi-inspired three-pane layout and git-backed notebooks"
  homepage "https://github.com/sazardev/shiki"
  version "0.8.10"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/sazardev/shiki/releases/download/v0.8.10/shiki-v0.8.10-aarch64-apple-darwin.tar.gz"
      sha256 "945584735f639460f71f70f3013dd4515b34ec3acce363540a99ae8d99d5d645"
    end
    on_intel do
      url "https://github.com/sazardev/shiki/releases/download/v0.8.10/shiki-v0.8.10-x86_64-apple-darwin.tar.gz"
      sha256 "2d725213ffa16bf7ec5f5508331eb53adb7f4e498269f7a902f246275688cc56"
    end
  end

  def install
    bin.install "shiki"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/shiki --version")
  end
end
