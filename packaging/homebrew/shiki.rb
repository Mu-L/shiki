class Shiki < Formula
  desc "TUI note-taking app with a Yazi-inspired three-pane layout and git-backed notebooks"
  homepage "https://github.com/sazardev/shiki"
  version "0.9.2"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.2/shiki-v0.9.2-aarch64-apple-darwin.tar.gz"
      sha256 "80a56c1bce47252b93a3b385d42b312dec787e6e20e3ce683896045500f010d7"
    end
    on_intel do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.2/shiki-v0.9.2-x86_64-apple-darwin.tar.gz"
      sha256 "a9cd8560b3471646ea67914a7a5e661644786775cb342aca7ee3df04e6b61b84"
    end
  end

  def install
    bin.install "shiki"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/shiki --version")
  end
end
