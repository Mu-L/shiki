class Shiki < Formula
  desc "TUI note-taking app with a Yazi-inspired three-pane layout and git-backed notebooks"
  homepage "https://github.com/sazardev/shiki"
  version "0.9.5"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.5/shiki-v0.9.5-aarch64-apple-darwin.tar.gz"
      sha256 "9ab81ac30f2a95e97601e63233bfe81f18abcceb413c2fa79ecaa8a9a8f0f775"
    end
    on_intel do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.5/shiki-v0.9.5-x86_64-apple-darwin.tar.gz"
      sha256 "cbe7d6330b262afb7990e82dfc03aae3e90a22d986456d750e91e03493a7db80"
    end
  end

  def install
    bin.install "shiki"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/shiki --version")
  end
end
