# Secret Project

## Dependencies
- `wasm-pack` for generating JavaScript/Wasm bindings:

    ```
    rustup target add wasm32-unknown-unknown
    cargo install wasm-pack
    ```

## Running

Build the client:
```
make clnt
```
This will generate the following files in `clnt/static/`:
- `clnt_bg.wasm.gz`
- `clnt.js.gz`

Build and run the server:
```
cargo run -j8 --bin serv -- \
    --clnt_dir clnt/static \
    --http_address <your-ip>:8080 \
    --webrtc_address <your-ip>:9000
```

## Credits
- `clnt/static/Munro-2LYe.ttf`: Munro font: http://www.tenbytwenty.com/
- `clnt/static/kongtext.ttf`: https://www.1001fonts.com/kongtext-font.html
- `clnt/static/ground.png`: By Tiziana, see https://opengameart.org/content/plain-concrete-256px
- Icons: https://game-icons.net
