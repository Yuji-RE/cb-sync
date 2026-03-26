{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "cb-sync-dev";

  buildInputs = with pkgs; [
    # Rust toolchain
    rustc
    cargo

    # Development tools
    rust-analyzer  # LSP for IDE support
    clippy         # Linter
    rustfmt        # Formatter

    # Runtime dependencies
    wl-clipboard   # wl-copy, wl-paste for Wayland

    # Useful for testing/debugging
    netcat-gnu     # nc for network testing
  ];

  # Reproducible builds
  RUST_BACKTRACE = "1";

  shellHook = ''
    echo "cb-sync development environment"
    echo "  cargo build    - Build the project"
    echo "  cargo test     - Run tests"
    echo "  cargo run      - Run CLI"
    echo "  cargo clippy   - Run linter"
    echo "  cargo fmt      - Format code"
  '';
}
