clnt: build-clnt package-clnt

clnt-debug: build-clnt-debug package-clnt

build-clnt:
	cd clnt && cargo build -j4 --release --target wasm32-unknown-unknown
	wasm-bindgen \
		--target web \
		--no-typescript \
		--out-dir clnt/pkg \
		target/wasm32-unknown-unknown/release/clnt.wasm

build-clnt-debug:
	cd clnt && cargo build -j4 --target wasm32-unknown-unknown
	wasm-bindgen \
		--target web \
		--no-typescript \
		--out-dir clnt/pkg \
		target/wasm32-unknown-unknown/debug/clnt.wasm

package-clnt:
	rollup clnt/pkg/clnt.js \
		--file clnt/static/clnt.js \
		--format iife \
		--name clnt
	cp clnt/pkg/clnt_bg.wasm clnt/static/
	gzip -f clnt/static/clnt_bg.wasm
	gzip -f clnt/static/clnt.js

.PHONY: clnt clnt-debug build-clnt build-clnt-debug package-clnt
