# Security

Please report a vulnerability privately rather than in a public issue: through
[a private advisory](https://github.com/sachesi/dethumb/security/advisories/new) on GitHub,
or by mail to sachesi <xsachesi@pm.me>. Say what you found, how to reproduce it and which
version you ran; a fix is worked out with you before anything is published.

Only the latest release gets fixes.

## What counts

A file manager runs dethumb by itself on `.desktop` files and Windows executables as soon
as a folder holding them is shown, so its input can come from anyone, such as a download
or an archive. dethumb never runs what it reads: it reads the icon an entry names, or the
icon resources in an executable. It has no sandbox of its own, so the limits below are all
that stands between such a file and your session unless the file manager adds one.

The parts where a mistake matters most:

- The parsers of PE resources, icon data, SVG and raster images. A file that makes dethumb
  run code, or get past its limits (64 MiB for an icon, 512 MiB for an executable, 8192
  pixels a side and 256 MiB of memory for decoding) is a vulnerability.
- The `Icon=` line of a desktop entry. It may only lead to an image file, never through a
  symbolic link. An entry that gets anything else read and shown as a thumbnail, or runs
  anything it names, is a vulnerability.
- The output. A way to make dethumb write outside the thumbnail path it was given, apart
  from the `.cachekey` file beside it, is a vulnerability.

A desktop entry pointing at a picture that you can read, and getting it shown as its
thumbnail, is what `Icon=` does and is not a vulnerability. A wrong or missing thumbnail
is a bug; please file it as an ordinary issue. A flaw in an image or SVG library belongs
with that library.
