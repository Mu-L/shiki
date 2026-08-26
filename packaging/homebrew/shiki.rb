class Shiki < Formula
  desc "TUI note-taking app with a Yazi-inspired three-pane layout and git-backed notebooks"
  homepage "https://github.com/sazardev/shiki"
  version "0.9.4"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.4/shiki-v0.9.4-aarch64-apple-darwin.tar.gz"
      sha256 "283f24a6e8def3a9b8d75814d3dbfb5d76b0620e32a15ae93556ee77ab71c997"
    end
    on_intel do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.4/shiki-v0.9.4-x86_64-apple-darwin.tar.gz"
      sha256 "fe69473020b24a3ffc00d756fc36c139596ed7e8945b3c08a33c19fa512aef74"
    end
  end

  def install
    bin.install "shiki"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/shiki --version")
  end
end
