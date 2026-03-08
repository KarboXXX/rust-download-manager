all: linux windows extension

windows:
	cargo build --target x86_64-pc-windows-gnu --release

linux:
	cargo build --target x86_64-unknown-linux-gnu --release

extension:
	$(MAKE) -C extension all
