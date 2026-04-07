#!/bin/bash

set -e

rustup override set nightly
rustup component add rust-src llvm-tools
cargo install bootimage
brew install qemu
rustup show
cargo bootimage --version
qemu-system-x86_64 --version
