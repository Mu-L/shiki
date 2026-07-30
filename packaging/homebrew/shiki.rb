class Shiki < Formula
  desc "TUI note-taking app with a Yazi-inspired three-pane layout and git-backed notebooks"
  homepage "https://github.com/sazardev/shiki"
  version "0.8.8"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/sazardev/shiki/releases/download/v0.8.8/shiki-v0.8.8-aarch64-apple-darwin.tar.gz"
      sha256 "3acbdeecf4687a12e5fe3f8b2631905c5d6bbfe8ae5e8856553abb441e12f485"
    end
    on_intel do
      url "https://github.com/sazardev/shiki/releases/download/v0.8.8/shiki-v0.8.8-x86_64-apple-darwin.tar.gz"
      sha256 "3658e94ab35e860b4a39494aa2c3ae76ed11838f464c3ba0d1a1adecb840ecde"
    end
  end

  def install
    bin.install "shiki"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/shiki --version")
  end
end
