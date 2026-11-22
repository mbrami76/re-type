# Re:Type – Building

Run the commands in the `re-type` directory.

## Building

You need Rust (stable, 1.75 or later) and, on Windows, either the MSVC build tools or the GNU toolchain.

```
cargo test                                                       # unit tests (any operating system)
cargo build --release                                            # target
elease
e-type.exe
```

To build the Windows executables from Linux, use zig as the linker (no Windows tools needed):

```
pip install ziglang && cargo install cargo-zigbuild
rustup target add x86_64-pc-windows-gnu
cargo zigbuild --release --target x86_64-pc-windows-gnu
```

The only third-party crates are `serde_json` (and its dependencies) and, on Windows, `windows-sys`. Windows code can be type-checked from Linux with `rustup target add x86_64-pc-windows-gnu` and `cargo check --target x86_64-pc-windows-gnu`.

## Automatic builds on GitHub

The workflow `.github/workflows/build.yml` tests the program and builds `re-type.exe` on every push and pull request, with pinned versions of Rust, Zig and cargo-zigbuild. Pushing a tag such as `v1.0.0` (it must match the version in `re-type/Cargo.toml`) attaches `re-type-1.0.0.exe` and its SHA-256 checksum to a GitHub release.
