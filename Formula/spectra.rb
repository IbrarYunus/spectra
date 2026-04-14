class Spectra < Formula
  desc "Fast terminal music visualizer with multiple styles"
  homepage "https://github.com/ibraryunus/spectra"
  license "MIT"
  head "https://github.com/ibraryunus/spectra.git", branch: "main"

  # For tagged releases, replace `head` usage and set url + sha256:
  # url "https://github.com/ibraryunus/spectra/archive/refs/tags/v0.1.0.tar.gz"
  # sha256 "..."
  # version "0.1.0"

  depends_on "rust" => :build
  depends_on :macos
  depends_on xcode: ["14.0", :build] # swiftc + ScreenCaptureKit SDK

  def install
    system "cargo", "build", "--release", "--locked"
    bin.install "target/release/spectra"
    lib.install "target/release/libspectra_sc.dylib"
  end

  test do
    assert_match "spectra", shell_output("#{bin}/spectra --version")
  end
end
