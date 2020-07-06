use log::{info, warn};

use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;

use quicksilver::input::Input;

use crate::{runner::Runner, webrtc};

#[derive(Debug, Clone)]
pub enum JoinAndConnectError {
    Request(JsValue),
    Join(comn::JoinError),
    WebRTC(webrtc::ConnectError),
}

pub async fn join_and_connect(
    request: comn::JoinRequest,
    input: &mut Input,
) -> Result<Runner, JoinAndConnectError> {
    let join_success = join_request(request)
        .await
        .map_err(JoinAndConnectError::Request)?
        .map_err(JoinAndConnectError::Join)?;

    let my_token = join_success.your_token;
    let on_message = Box::new(
        move |client_data: &webrtc::Data, message: &comn::ServerMessage| {
            on_message(my_token, client_data, message)
        },
    );
    let webrtc_client = webrtc::Client::connect(Default::default(), on_message)
        .await
        .map_err(JoinAndConnectError::WebRTC)?;

    while webrtc_client.status() == webrtc::Status::Connecting {
        info!("Waiting...");
        webrtc_client.debug_ready_state();

        // Note: this is here as a way to yield control back to JavaScript.
        // There probably is a better way to do this.
        input.next_event().await;

        // TODO: Timeout
    }

    Ok(Runner::new(join_success, webrtc_client))
}

pub async fn join_request(request: comn::JoinRequest) -> Result<comn::JoinReply, JsValue> {
    let request_json = format!(
        "{{\"game_id\":{},\"player_name\":\"{}\"}}",
        request
            .game_id
            .map_or("null".to_owned(), |comn::GameId(id)| "\"".to_owned()
                + &id.to_string()
                + "\""),
        request.player_name,
    );

    let mut opts = web_sys::RequestInit::new();
    opts.method("POST");
    opts.mode(web_sys::RequestMode::SameOrigin);
    opts.body(Some(&JsValue::from_str(&request_json)));

    info!("Requesting to join game: {} ...", request_json);

    let request = web_sys::Request::new_with_str_and_init(&"/join", &opts)?;
    request.headers().set("Accept", "application/json")?;

    let window = web_sys::window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    assert!(resp_value.is_instance_of::<web_sys::Response>());
    let resp: web_sys::Response = resp_value.dyn_into().unwrap();

    // Convert this other `Promise` into a rust `Future`.
    let reply = JsFuture::from(resp.json()?).await?;

    info!("Join reply: {:?}", reply);

    // Use serde to parse the JSON into a struct.
    Ok(reply.into_serde().unwrap())
}

pub fn on_message(
    my_token: comn::PlayerToken,
    client_data: &webrtc::Data,
    message: &comn::ServerMessage,
) {
    if let comn::ServerMessage::Ping(sequence_num) = message {
        let reply = comn::ClientMessage::Pong(*sequence_num);
        let signed_message = comn::SignedClientMessage(my_token, reply);
        let data = signed_message.serialize();
        if let Err(err) = client_data.send(&data) {
            warn!("Failed to send message: {:?}", err);
        }
    }
}
