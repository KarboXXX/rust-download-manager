all: linux windows

windows:
	cargo build --target x86_64-pc-windows-gnu --release

linux:
	cargo build --target x86_64-unknown-linux-gnu --release
