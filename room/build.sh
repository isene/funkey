#!/bin/bash
# Builds the game room's games for the web, one build each, into
# room/wasm/<game>.wasm. With names, builds only those.
#
#   room/build.sh [game...]
set -e
here=$(cd "$(dirname "$0")" && pwd)
top=$(cd "$here/../.." && pwd)
cd "$here/../web"
for g in ${@:-raid kart salvo wargame stack again marble chomp eliminator vector blocks jumpman}; do
  PATH="/usr/bin:$PATH" RUSTFLAGS="--remap-path-prefix=$HOME/= --remap-path-prefix=$top/=" \
    cargo build --release --quiet --target wasm32-unknown-unknown --features "$g"
  /usr/bin/cp target/wasm32-unknown-unknown/release/funkey_web.wasm "$here/wasm/$g.wasm"
  # A file that names a folder on this machine is not published.
  if grep -qa -e "$HOME" -e "${top#"$HOME"/}" "$here/wasm/$g.wasm"; then echo "$g.wasm carries a local path"; exit 1; fi
  echo "$g $(stat -c %s "$here/wasm/$g.wasm")"
done
