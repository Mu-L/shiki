class Shiki < Formula
  desc "TUI note-taking app with a Yazi-inspired three-pane layout and git-backed notebooks"
  homepage "https://github.com/sazardev/shiki"
  version "0.9.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.1/shiki-v0.9.1-aarch64-apple-darwin.tar.gz"
      sha256 "2b38e402105dd90ba9fcedf3b9d4d1185815d942903247630a93e26b28c114b3"
    end
    on_intel do
      url "https://github.com/sazardev/shiki/releases/download/v0.9.1/shiki-v0.9.1-x86_64-apple-darwin.tar.gz"
      sha256 "9a036a9ea339f2e01208ce2f05758a58ffc8c365b1e9ac6e928269420a48bac9"
    end
  end

  def install
    bin.install "shiki"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/shiki --version")
  end
end
