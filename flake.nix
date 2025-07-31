{
  description = "mooR - Network-accessible, multi-user, programmable system for building online social environments";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain matching the project's rust-version
        rustToolchain = pkgs.rust-bin.stable."1.88.0".default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
        };

        # Development dependencies needed for the project
        buildInputs = with pkgs; [
          # Core Rust toolchain
          rustToolchain
          
          # System dependencies for mooR
          openssl
          pkg-config
          sqlite
          
          # ZMQ for networking
          zeromq
          
          # For debian package building
          cargo-deb
          
          # Docker for testing
          docker
          docker-compose
          
          # Additional development tools
          git
          just
          bacon
        ];

        nativeBuildInputs = with pkgs; [
          pkg-config
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;
          
          # Environment variables
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig:${pkgs.sqlite.dev}/lib/pkgconfig";
          OPENSSL_DIR = "${pkgs.openssl.dev}";
          OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
          
          shellHook = ''
            echo "🦀 mooR development environment"
            echo "Rust version: $(rustc --version)"
            echo "Cargo version: $(cargo --version)"
            echo ""
            echo "🚀 Development workflows:"
            echo ""
            echo "📦 Building & Testing:"
            echo "  cargo build                    - Build the project"
            echo "  cargo test                     - Run tests"
            echo "  cargo deb                      - Build debian packages"
            echo ""
            echo "🔧 Development servers (pick one):"
            echo ""
            echo "  🥓 Bacon (file watching):"
            echo "    bacon daemon                 - Run daemon with hot reload"
            echo "    bacon telnet                 - Run telnet host with hot reload"
            echo "    bacon web                    - Run web host with hot reload"
            echo "    bacon test                   - Run tests with hot reload"
            echo ""
            echo "  🔄 Process Compose (all services):"
            echo "    process-compose up           - Start all services"
            echo "    process-compose down         - Stop all services"
            echo "    • Daemon + Telnet (8888) + Web (8080) + Worker"
            echo ""
            echo "  🐳 Docker Compose (containerized):"
            echo "    docker-compose up            - Start containerized stack"
            echo "    docker-compose down          - Stop containers"
            echo "    • Production-like setup with networking"
            echo ""
            echo "🌐 Access points:"
            echo "  • Telnet: telnet localhost 8888"
            echo "  • Web: http://localhost:8080"
          '';
        };

        # Optional: provide packages for direct installation
        packages = {
          # Make the rust toolchain available as a package
          rust = rustToolchain;
          
          # Provide cargo-deb as a standalone package
          cargo-deb = pkgs.cargo-deb;
        };

        # Default package points to the rust toolchain
        packages.default = rustToolchain;
      });
}