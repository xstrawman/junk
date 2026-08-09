# Homebrew formula for junk
# Install (tap this repo):
#   brew tap xstrawman/junk https://github.com/xstrawman/junk
#   brew install junk
#
# Or one-shot from raw formula (build from source):
#   brew install --build-from-source ./Formula/junk.rb
#
# HEAD (latest main):
#   brew install --HEAD junk

class Junk < Formula
  desc "Super-fast multi-connection HTTP(S) downloader with arcade TUI"
  homepage "https://github.com/xstrawman/junk"
  license "MIT"
  version "0.1.1"

  # Stable source archive — sha256 filled for the v0.1.1 tag
  url "https://github.com/xstrawman/junk/archive/refs/tags/v0.1.1.tar.gz"
  sha256 "f1f296e0994f0b67a04c2feeadcad90744a5c92198ca13434c74b4255ddb0d55"

  head "https://github.com/xstrawman/junk.git", branch: "master"

  depends_on "rust" => :build

  on_linux do
    depends_on "pkgconf" => :build
  end

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/junk")
  end

  def caveats
    <<~EOS
      Quick start:
        junk                          # arcade TUI (a = add URL from clipboard)
        junk https://example.com/f    # CLI multi-conn download
        junk --ventoy https://…/iso   # distrohopper: ISO → Ventoy

      Default download dir: ~/Downloads (override with -d DIR)
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/junk --version")
    assert_match "ventoy", shell_output("#{bin}/junk --help")
  end
end
