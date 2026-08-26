class Shiki < Formula
  desc "TUI note-taking app with a Yazi-inspired three-pane layout and git-backed notebooks"
  homepage "https://github.com/sazardev/shiki"
  version "0.9.3"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.3/shiki-v0.9.3-aarch64-apple-darwin.tar.gz"
      sha256 "ce1f63fa084a3e709bed504280afe5a4f6cae9694783c61c321cc9daee53a06c"
    end
    on_intel do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.3/shiki-v0.9.3-x86_64-apple-darwin.tar.gz"
      sha256 "6dfb064f21f0d9e6d8eb72865e023d0aa03ad28d7c34c6a81ababe0081213093"
    end
  end

  def install
    bin.install "shiki"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/shiki --version")
  end
end
