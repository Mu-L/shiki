class Shiki < Formula
  desc "TUI note-taking app with a Yazi-inspired three-pane layout and git-backed notebooks"
  homepage "https://github.com/sazardev/shiki"
  version "0.9.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.0/shiki-v0.9.0-aarch64-apple-darwin.tar.gz"
      sha256 "237b5d605dbd21563d3657de4c0111c71153e6096fff654487c379511d6a695b"
    end
    on_intel do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.0/shiki-v0.9.0-x86_64-apple-darwin.tar.gz"
      sha256 "28500ed8f9a88f0eacd576266919e7a249961a6404ea807f2f983293e8efc424"
    end
  end

  def install
    bin.install "shiki"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/shiki --version")
  end
end
