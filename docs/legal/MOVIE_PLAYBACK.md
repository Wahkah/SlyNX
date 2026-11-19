# Movie playback: licensing and patents

Status: decision record, 2026-11-13. Not legal advice; **[verify online]** marks facts that were not checked
against a primary source, and **[lawyer]** marks points for a lawyer.

## The question

Sly 1's 17 movies are MPEG-2 video (Main Profile @ Main Level, 640x480, interlaced) in a PSS container (an MPEG-2
program stream), with raw 16-bit PCM audio (`specs/sly1/movies.md`). The Switch build has to play them. Three ways:

- **(a)** Our own MPEG-2 video decoder in Rust, written from the public standard.
- **(b)** FFmpeg (libavcodec) statically linked into the Switch `.nro`.
- **(c)** The packager on the player's PC calls an FFmpeg the player installed, to convert the movies into a simpler
  format that we decode ourselves.

## Decision: (a), our own decoder

### Licensing

- Our own code, under the project license (MIT OR Apache-2.0). No third-party code, no notices, no copyleft.
- Written from **ITU-T H.262** (technically identical to ISO/IEC 13818-2; the ITU edition is distributed free of
  charge by the ITU **[verify online]**) and ISO/IEC 13818-1 / H.222.0 for the program stream. The code tables,
  default quantiser matrices and scan orders are functional parts of the standard that any decoder must reproduce
  to interoperate; using them as the standard gives them is normal practice and low risk.
- Clean-room rule ([CLEAN_ROOM.md](CLEAN_ROOM.md), section 6): whoever writes the decoder does not read FFmpeg,
  libmpeg2, PCSX2's IPU code, the reference projects' movie code, or any other decoder's source. Running FFmpeg on a
  developer's PC to produce reference frames for image comparison is fine (a test oracle; nothing is copied or
  shipped).
- The PSS container and the audio layout are described in the spec from the disc data; raw PCM needs no codec.

### Patents

- MPEG-2 video was covered by the MPEG LA (now Via LA) MPEG-2 patent pool. The last US patents in that pool expired
  in **February 2018**, which is when several Linux distributions started shipping MPEG-2 decoders by default
  **[verify online]**. Public reports put the expiry of the remaining patents in other countries at or before the
  early 2020s **[verify online: the pool's final patent list and the last expiry date per country]**.
- Raw PCM audio and the MPEG-2 systems layer are not a patent concern known to us.
- If the check shows any patent still in force somewhere, the practical risk for a free, open-source,
  non-commercial decoder is very low, but note it here. **[lawyer]** only if the project ever becomes commercial.

### Cost

A decoder for one profile (MP@ML, 4:2:0, no scalability) is bounded work: bitstream and start codes, the variable
length code tables, inverse quantisation, an 8x8 IDCT that meets the standard's accuracy requirement, motion
compensation (frame and field prediction, since the movies are interlaced), and frame reordering. 640x480 at 30
frames per second is well within the Switch CPU's budget, especially with NEON. Since crates cannot be added in this
environment, it has to be written in-house anyway.

## Why not (b), FFmpeg linked into the `.nro`

- **The FFmpeg installed with devkitPro is a GPL build.** Its configuration (embedded in `libavcodec.a`) includes
  `--enable-gpl`, and the package metadata says `GPL`. Linking it would make every Switch binary we publish
  distributable only under the GPL as a whole. Our MIT code can be combined into a GPL binary, but each release would
  then have to ship the complete corresponding source of the combination (FFmpeg's included, with the exact build
  configuration) and the GPL text, and the binary would carry everything that build enables (libass, freetype,
  dav1d, network protocols).
- **An LGPL build would have to be built by us** (`--disable-everything --enable-decoder=mpeg2video
  --enable-demuxer=mpegps`, no `--enable-gpl`), from FFmpeg source that we cannot download here. Static linking
  then triggers LGPL-2.1 section 6: users must be able to relink with a modified FFmpeg. As our own source is
  public, that is satisfiable (source plus build instructions, plus FFmpeg's source or a written offer), but it is a
  standing obligation on every release.
- **Patent exposure grows with the build.** A general FFmpeg build ships decoders for formats whose patents are still
  in force in some countries (H.264, HEVC, AAC). Distributing those binaries is a risk we avoid by not shipping them.
- It is a large C dependency against the "thin C layer" design, and it does not remove the need to demux PSS and
  sync audio ourselves.

## Why not (c), the player's FFmpeg converts the movies

- **Licensing is clean** (we distribute nothing of FFmpeg; the player runs their own copy), but:
- **It does not remove the decoder.** The converted movies still need a codec on the Switch. Uncompressed frames are
  far too large (about 460 KB per 640x480 4:2:0 frame, so around 14 MB per second of video). A simpler codec
  (Motion JPEG) still means writing a decoder and makes the files several times larger. AV1 with dav1d (BSD-2-Clause,
  available in devkitPro) is royalty-free but adds a C dependency and needs an AV1 encoder in the player's FFmpeg.
- **It adds a step for every player** (installing FFmpeg, version differences across systems) and re-encoding loses
  quality.

Option (c) stays as a fallback only if (a) is late: then the packager could convert movies for the PC build while
the Switch build skips movies.

## Follow-ups

1. The owner downloads ITU-T H.262 and H.222.0 from the ITU (not possible from this environment) and keeps them
   outside the repository, with the edition and date recorded in `specs/sly1/movies.md` `sources:`.
2. Confirm the MPEG-2 patent expiry from a primary source (the pool's final patent list) and record the result
   here.
3. Add FFmpeg, libmpeg2 and PCSX2's IPU sources to the similarity audit's reference set when the decoder lands.
