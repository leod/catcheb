# Secret Project

Multiplayer game with web clients that connect to an authoritative server through WebRTC.

Video of a stress test:

[![stress test](https://img.youtube.com/vi/vOkd2RIo32U/0.jpg)](https://www.youtube.com/watch?v=vOkd2RIo32U)

I've abandoned this project, but learnings will go into the next iteration, which is already work in progress!

## Dependencies
- `npm` (only needed for the `rollup` packer):
    Installation on Ubuntu (otherwise, follow https://github.com/nodesource/distributions/blob/master/README.md)

    ```
    curl -sL https://deb.nodesource.com/setup_14.x | sudo -E bash -
    sudo apt-get install -y nodejs
    ```
- `wasm-pack` for generating JavaScript/Wasm bindings:

    ```
    rustup target add wasm32-unknown-unknown
    cargo install wasm-pack
    ```
- `rollup` for combining JavaScript files into a single file:

    ```
    sudo npm install --global rollup
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

## Useful resources
- https://dev.to/dandyvica/wasm-in-rust-without-nodejs-2e0c

## Credits
- `clnt/static/Munro-2LYe.ttf`: Munro font: http://www.tenbytwenty.com/
- `clnt/static/kongtext.ttf`: https://www.1001fonts.com/kongtext-font.html
- `clnt/static/ground.png`: By Tiziana, see https://opengameart.org/content/plain-concrete-256px
- Icons: https://game-icons.net
