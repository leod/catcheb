clnt:
	cd clnt && cargo build --target wasm32-unknown-unknown
	wasm-bindgen \
		--target web \
		--no-typescript \
		--out-dir clnt/pkg \
		target/wasm32-unknown-unknown/debug/clnt.wasm
	rollup clnt/pkg/clnt.js \
		--file clnt/static/clnt.js \
		--format iife \
		--name clnt
	cp clnt/pkg/clnt_bg.wasm clnt/static/

.PHONY: clnt
