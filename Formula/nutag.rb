class Nutag < Formula
  desc "Command-line tool for creating and managing semantic version tags in Git and Jujutsu"
  homepage "https://github.com/felipesere/nutag"
  version "0.1.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/felipesere/nutag/releases/download/v#{version}/nutag-macos-aarch64.tar.gz"
      sha256 "900a8f5bf02ba8acc39b163a55ff926ff77c6bdf96599c371c56cefd81ad2f6a"
    else
      url "https://github.com/felipesere/nutag/releases/download/v#{version}/nutag-macos-x86_64.tar.gz"
      sha256 "88461dc09cc06a9defcc6c81728109e8c3261d0ddc24e411b764e19987a2d4dd"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/felipesere/nutag/releases/download/v#{version}/nutag-linux-aarch64.tar.gz"
      sha256 "0eb16b49963adf84666ba2335742b6b3098ae2cd23376565c93e88c7181dd650"
    else
      url "https://github.com/felipesere/nutag/releases/download/v#{version}/nutag-linux-x86_64.tar.gz"
      sha256 "21149d0bf2c7d35e91d36c44bd0c451ba39020961900f540e64831744ecbc5a8"
    end
  end

  def install
    bin.install "nutag"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/nutag --version 2>&1", 2)
  end
end
