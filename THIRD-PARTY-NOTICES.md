# Third-party notices

Re:Type is free software under the GNU General Public License, version 3 or (at your option) any later version (see `LICENSE`). It links the Rust libraries below. Each is distributed under the licence stated in its own package metadata; all of them are permissive licences that are compatible with the GPL. The versions are those in `re-type/Cargo.lock`.

| Library | Version | Licence | Used for |
|---|---|---|---|
| serde_json | 1.0.154 | MIT OR Apache-2.0 | reading and writing the settings file and language files |
| serde | 1.0.229 | MIT OR Apache-2.0 | dependency of serde_json |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | dependency of serde_json |
| itoa | 1.0.19 | MIT OR Apache-2.0 | number formatting, dependency of serde_json |
| memchr | 2.8.3 | Unlicense OR MIT | byte search, dependency of serde_json |
| zmij | 1.0.23 | MIT | number formatting, dependency of serde_json |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | Windows API declarations (Windows build only) |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | linking, dependency of windows-sys (Windows build only) |

The Rust standard library, which is linked into every build, is distributed under MIT OR Apache-2.0. The Windows executables are built with the Zig toolchain's linker and its bundled mingw-w64 runtime, which carry permissive licences.

Where a library offers a choice of licences ("OR"), Re:Type uses it under the MIT licence (or the Unlicense for memchr). The full licence texts are in each package's source distribution (for example under `~/.cargo/registry/src` after `cargo fetch`) and at <https://spdx.org/licenses/>.

## Build-time only (not distributed)

- The icons were drawn with a Python script (`re-type/assets/make-icons.py`) that uses the Pillow library and the DejaVu Sans Bold font to render the letters "RE:". Only the resulting pictures are distributed. DejaVu fonts may be used and embedded freely under their licence (Bitstream Vera licence with public-domain additions).
