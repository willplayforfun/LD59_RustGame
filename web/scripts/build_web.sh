#!/usr/bin/env bash
set -e

OUTDIR="web/Survey_web"

echo "Building WASM..."
cargo build --release --target wasm32-unknown-unknown

echo "Packaging..."
rm -rf "$OUTDIR"
mkdir "$OUTDIR"
cp target/wasm32-unknown-unknown/release/Survey.wasm "$OUTDIR/"
cp web/bundle/index.html "$OUTDIR/"
cp web/bundle/mq_js_bundle.js "$OUTDIR/"

echo "Zipping..."
rm -f web/Survey_web.zip
zip -r web/Survey_web.zip "$OUTDIR"

echo "Done -> web/Survey_web.zip"
