//! Unreliable WebRTC client.
//!
//! This is based on the `echo_server.html` example from `webrtc-unreliable`,
//! but translated from JavaScript into Rust.

use log::info;

use js_sys::{Reflect, JSON};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    RtcConfiguration, RtcDataChannel, RtcDataChannelInit, RtcPeerConnection,
    RtcSessionDescriptionInit,
};

#[derive(Debug, Clone)]
pub enum ConnectionError {
    NewRtcPeerConnection(JsValue),
    CreateOffer(JsValue),
    SetLocalDescription(JsValue),
    NewRequest(JsValue),
    Fetch(JsValue),
    ResponseStatus(u16),
    ResponseJson(JsValue),
    SetRemoteDescription(JsValue),
    AddIceCandidate(JsValue),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub address: String,
    pub ice_server_urls: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: "/connect_webrtc".to_string(),
            ice_server_urls: vec![
                "stun:stun.l.google.com:19302".to_string(),
                /*"stun:stun1.l.google.com:19302".to_string(),
                "stun:stun2.l.google.com:19302".to_string(),
                "stun:stun3.l.google.com:19302".to_string(),
                "stun:stun4.l.google.com:19302".to_string(),*/
            ],
        }
    }
}

pub struct Client {}

impl Client {
    pub async fn connect(config: Config) -> Result<Self, ConnectionError> {
        info!("Establishing WebRTC connection");

        let ice_servers: JsValue = {
            let json = "[{\"urls\":[".to_string()
                + &config
                    .ice_server_urls
                    .iter()
                    .map(|url| "\"".to_string() + url + "\"")
                    .collect::<Vec<_>>()
                    .join(",")
                + "]}]";
            JSON::parse(&json).unwrap()
        };

        info!("WebRTC ICE servers: {:?}", ice_servers);

        let rtc_configuration: RtcConfiguration = {
            let mut v = RtcConfiguration::new();
            v.ice_servers(&ice_servers);
            v
        };

        let peer: RtcPeerConnection = RtcPeerConnection::new_with_configuration(&rtc_configuration)
            .map_err(ConnectionError::NewRtcPeerConnection)?;

        let data_channel_init: RtcDataChannelInit = {
            let mut v = RtcDataChannelInit::new();
            v.ordered(false).max_retransmits(0);
            v
        };

        let channel: RtcDataChannel =
            peer.create_data_channel_with_data_channel_dict("webudp", &data_channel_init);

        let offer: RtcSessionDescriptionInit = JsFuture::from(peer.create_offer())
            .await
            .map_err(ConnectionError::CreateOffer)?
            .into();

        JsFuture::from(peer.set_local_description(&offer))
            .await
            .map_err(ConnectionError::SetLocalDescription)?;

        // Request a session from the WebRTC server
        let reply: JsValue = {
            let mut opts = web_sys::RequestInit::new();
            opts.method("POST");
            opts.mode(web_sys::RequestMode::SameOrigin);
            opts.body(Some(
                &Reflect::get(&offer, &JsValue::from_str("sdp")).unwrap(),
            ));

            info!("Requesting WebRTC session...");

            let request = web_sys::Request::new_with_str_and_init(&config.address, &opts)
                .map_err(ConnectionError::NewRequest)?;

            let window = web_sys::window().unwrap();
            let response_value: JsValue = JsFuture::from(window.fetch_with_request(&request))
                .await
                .map_err(ConnectionError::Fetch)?;
            assert!(response_value.is_instance_of::<web_sys::Response>());
            let response: web_sys::Response = response_value.dyn_into().unwrap();

            if (response.status() != 200) {
                return Err(ConnectionError::ResponseStatus(response.status()));
            }

            let reply: JsValue =
                JsFuture::from(response.json().map_err(ConnectionError::ResponseJson)?)
                    .await
                    .map_err(ConnectionError::ResponseJson)?;

            info!("Reply: {:?}", reply);

            reply
        };

        let (answer, candidate) = (
            Reflect::get(&reply, &JsValue::from_str("answer"))
                .map_err(ConnectionError::ResponseJson)?,
            Reflect::get(&reply, &JsValue::from_str("candidate"))
                .map_err(ConnectionError::ResponseJson)?,
        );

        JsFuture::from(peer.set_remote_description(&answer.into()))
            .await
            .map_err(ConnectionError::SetRemoteDescription)?;

        JsFuture::from(peer.add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate.into())))
            .await
            .map_err(ConnectionError::AddIceCandidate)?;

        Ok(Client {})
    }
}
