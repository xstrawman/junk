# Homebrew formula for junk
#   brew tap xstrawman/junk https://github.com/xstrawman/junk
#   brew install junk
#   brew install --HEAD junk

class Junk < Formula
  desc "Hypersonic multi-conn downloader + streams (aria2 × ytdl × ffmpeg)"
  homepage "https://github.com/xstrawman/junk"
  license "MIT"
  version "0.2.0"

  url "https://github.com/xstrawman/junk/archive/refs/tags/v0.2.0.tar.gz"
  sha256 "REPLACE_AFTER_TAG"

  head "https://github.com/xstrawman/junk.git", branch: "master"

  depends_on "rust" => :build
  depends_on "ffmpeg" => :recommended
  depends_on "yt-dlp" => :recommended

  on_linux do
    depends_on "pkgconf" => :build
  end

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/junk")
  end

  def caveats
    <<~EOS
      junk = multi-conn HTTP × yt-dlp streams × ffmpeg

        junk                          # clipboard URL → download (ASCII)
        junk <url>                    # file or stream auto-detect
        junk --audio <url>            # MP3 → ~/Music
        junk tui                      # arcade TUI (great on Mac)
        junk --ventoy https://…/iso   # ISO → Ventoy

      Streaming sites need yt-dlp + ffmpeg (brew install yt-dlp ffmpeg).
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/junk --version")
    assert_match "stream", shell_output("#{bin}/junk --help")
  end
end
