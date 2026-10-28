class AgentDesktop < Formula
  desc "Native accessibility-based desktop automation for agents"
  homepage "https://github.com/lahfir/agent-desktop"
  version "1.0.0"
  license "Apache-2.0"

  on_macos do
    depends_on macos: :ventura

    on_arm do
      url "https://github.com/lahfir/agent-desktop/releases/download/v1.0.0/agent-desktop-v1.0.0-aarch64-apple-darwin.tar.gz"
      sha256 "8e8e8e4d3bbe0f769f23742f113dd428427d022d709fee1ee25a726bb5c45317"
    end

    on_intel do
      url "https://github.com/lahfir/agent-desktop/releases/download/v1.0.0/agent-desktop-v1.0.0-x86_64-apple-darwin.tar.gz"
      sha256 "ed861e0fca413b604a7895190b339e5289fdbc9e8e7e42bdee9aba1ef6bc5014"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/lahfir/agent-desktop/releases/download/v1.0.0/agent-desktop-v1.0.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "d7dfeebc63905c1de678291b5ac695d8e85ea17adf801245738a569a3182cf64"
    end

    on_intel do
      url "https://github.com/lahfir/agent-desktop/releases/download/v1.0.0/agent-desktop-v1.0.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "5398a1c12f88b0d19a14f37be11cdb69370ab0a1aea839a69c8a074c6388de43"
    end
  end

  def install
    bin.install "agent-desktop"
    bin.install "agent-desktop-macos-helper" if OS.mac?
    bin.install "agent-desktop-linux-clipboard-owner" if OS.linux?
  end

  test do
    require "json"
    response = JSON.parse(shell_output("#{bin}/agent-desktop version"))
    assert_equal true, response.fetch("ok")
    assert_equal version.to_s, response.fetch("data").fetch("version")
    assert_equal OS.mac? ? "macos" : "linux", response.fetch("data").fetch("os")
    assert_path_exists bin/(OS.mac? ? "agent-desktop-macos-helper" : "agent-desktop-linux-clipboard-owner")
  end
end
