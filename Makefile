lint:
	cargo fmt
	cargo clippy --tests -- -D warnings
