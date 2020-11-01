clnt: build-clnt package-clnt

clnt-debug: build-clnt-debug package-clnt

build-clnt:
	cd clnt && wasm-pack build --target web --no-typescript --release

build-clnt-debug:
	cd clnt && wasm-pack build --target web --no-typescript --dev

package-clnt:
	cp clnt/pkg/clnt.js clnt/static/clnt.js 
	cp clnt/pkg/clnt_bg.wasm clnt/static/
	gzip -f clnt/static/clnt_bg.wasm
	gzip -f clnt/static/clnt.js

.PHONY: clnt clnt-debug build-clnt build-clnt-debug package-clnt
