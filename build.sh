#!/usr/bin/env bash

## Building
cargo build --target x86_64-pc-windows-gnu --release
cargo build --target x86_64-unknown-linux-gnu --release

## Generating MD5 checksum
md5sum --tag ./target/x86_64-unknown-linux-gnu/release/karbox_downloader
md5sum --tag ./target/x86_64-pc-windows-gnu/release/karbox_downloader.exe
