
dev:
	cli-dev-init
	cli-dev-deploy

dev-deploy:
	cargo run deploy -p ./src/beav

dev-init:
	cargo run init --force


release:
	cargo build --release

docs:
	cargo doc --no-deps --all-features --document-private-items


