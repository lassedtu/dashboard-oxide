#!/usr/bin/env bash
# Install (or uninstall) the `dbx` command.
#
#   ./install.sh              build a release binary and install it
#   ./install.sh --uninstall  remove the installed binary (config is kept)
#
# Installs via `cargo install` into $CARGO_INSTALL_ROOT/bin, or ~/.cargo/bin
# by default. Set CARGO_INSTALL_ROOT to install somewhere else.

set -euo pipefail

cd "$(dirname "$0")"

bold=$'\033[1m' green=$'\033[32m' yellow=$'\033[33m' red=$'\033[31m' reset=$'\033[0m'
info() { printf '%s==>%s %s\n' "$bold" "$reset" "$*"; }
ok()   { printf '%s✓%s %s\n' "$green" "$reset" "$*"; }
warn() { printf '%s!%s %s\n' "$yellow" "$reset" "$*"; }
die()  { printf '%s✗%s %s\n' "$red" "$reset" "$*" >&2; exit 1; }

bin_dir="${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin"

command -v cargo >/dev/null 2>&1 ||
    die "cargo not found. Install Rust first: https://rustup.rs"

case "${1:-}" in
    "") ;;
    --uninstall)
        info "Uninstalling dbx"
        cargo uninstall dbx
        ok "Removed dbx from $bin_dir (config in ~/.dbx was left alone)"
        exit 0
        ;;
    *) die "unknown argument: $1 (expected nothing or --uninstall)" ;;
esac

info "Building and installing dbx (release)"
cargo install --path . --locked --force --quiet

installed="$bin_dir/dbx"
[[ -x "$installed" ]] || die "install finished but $installed is missing"
ok "Installed $("$installed" --version) to $installed"

# Make sure typing `dbx` actually runs what we just installed.
case ":$PATH:" in
    *":$bin_dir:"*)
        resolved="$(command -v dbx || true)"
        if [[ "$resolved" != "$installed" ]]; then
            warn "\`dbx\` resolves to $resolved, which shadows $installed"
            warn "Remove it or move $bin_dir earlier in your PATH."
        else
            ok "Run \`dbx\` to start"
        fi
        ;;
    *)
        warn "$bin_dir is not on your PATH. Add this to your shell config (e.g. ~/.zshrc):"
        printf '\n    export PATH="%s:$PATH"\n\n' "$bin_dir"
        ;;
esac
