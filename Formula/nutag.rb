class Nutag < Formula
  desc "Command-line tool for creating and managing semantic version tags in Git and Jujutsu"
  homepage "https://github.com/felipesere/nutag"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/felipesere/nutag/releases/download/v#{version}/nutag-macos-aarch64.tar.gz"
      sha256 "" # Will be updated automatically on release
    else
      url "https://github.com/felipesere/nutag/releases/download/v#{version}/nutag-macos-x86_64.tar.gz"
      sha256 "" # Will be updated automatically on release
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/felipesere/nutag/releases/download/v#{version}/nutag-linux-aarch64.tar.gz"
      sha256 "" # Will be updated automatically on release
    else
      url "https://github.com/felipesere/nutag/releases/download/v#{version}/nutag-linux-x86_64.tar.gz"
      sha256 "" # Will be updated automatically on release
    end
  end

  def install
    bin.install "nutag"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/nutag --version 2>&1", 2)
  end
end
