# Secret Project

## Running
Build the client:
```
cd clnt && cargo web deploy -j8 && cd ../
```
This will create the files `clnt.wasm`, `clnt.js` and `index.html` in `target/deploy/`.

Build and run the server:
```
cargo run -j8 --bin serv -- --clnt_deploy_dir target/deploy/ --http_address <your-ip>:8080
```