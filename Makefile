clnt: build-clnt package-clnt

clnt-debug: build-clnt-debug package-clnt

build-clnt:
	cd clnt && wasm-pack build --mode no-install --target web --no-typescript --release

build-clnt-debug:
	cd clnt && wasm-pack build --mode no-install --target web --no-typescript 

package-clnt:
	rollup clnt/pkg/clnt.js \
		--file clnt/static/clnt.js \
		--format iife \
		--name clnt
	cp clnt/pkg/clnt_bg.wasm clnt/static/
	gzip -f clnt/static/clnt_bg.wasm
	gzip -f clnt/static/clnt.js

.PHONY: clnt clnt-debug build-clnt build-clnt-debug package-clnt
