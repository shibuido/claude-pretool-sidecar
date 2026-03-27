class ClaudePretoolSidecar < Formula
  desc "Composable sidecar for Claude Code hooks that aggregates tool-approval votes"
  homepage "https://github.com/shibuido/claude-pretool-sidecar"
  url "https://github.com/shibuido/claude-pretool-sidecar/archive/v0.1.0.tar.gz"
  sha256 "PLACEHOLDER_SHA256"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    # Verify all binaries are installed and respond to --help
    assert_match "claude-pretool-sidecar", shell_output("#{bin}/claude-pretool-sidecar --help 2>&1", 0)
    assert_match "logger", shell_output("#{bin}/claude-pretool-logger --help 2>&1", 0)
    assert_match "analyzer", shell_output("#{bin}/claude-pretool-analyzer --help 2>&1", 0)
  end
end
