//! Unreliable WebRTC client.
//!
//! This is based on the `echo_server.html` example from `webrtc-unreliable`,
//! but translated from JavaScript into Rust.

use std::{cell::Cell, rc::Rc};

use log::{info, warn};

use js_sys::{Reflect, JSON};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    ErrorEvent, Event, MessageEvent, RtcConfiguration, RtcDataChannel, RtcDataChannelInit,
    RtcPeerConnection, RtcSessionDescriptionInit,
};

#[derive(Debug, Clone)]
pub enum ConnectError {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Open,
    Closed,
    Error,
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

pub struct Client {
    status: Rc<Cell<Status>>,
    _on_open: Closure<dyn FnMut(&Event)>,
    _on_close: Closure<dyn FnMut(&Event)>,
    _on_error: Closure<dyn FnMut(&ErrorEvent)>,
}

impl Client {
    pub async fn connect(config: Config) -> Result<Self, ConnectError> {
        info!("Establishing WebRTC connection");

        let peer: RtcPeerConnection = new_rtc_peer_connection(&config)?;
        let channel: RtcDataChannel = create_data_channel(&peer);

        let status = Rc::new(Cell::new(Status::Connecting));

        let on_open = Closure::wrap(Box::new({
            let status = status.clone();
            move |_: &Event| on_open(status.clone())
        }) as Box<dyn FnMut(&Event)>);
        channel.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        // TODO: We'll also want to handle close events caused by the peer
        let on_close = Closure::wrap(Box::new({
            let status = status.clone();
            move |_: &Event| on_close(status.clone())
        }) as Box<dyn FnMut(&Event)>);
        channel.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        let on_error = Closure::wrap(Box::new({
            let status = status.clone();
            move |event: &ErrorEvent| on_error(status.clone(), event)
        }) as Box<dyn FnMut(&ErrorEvent)>);
        channel.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        let offer: RtcSessionDescriptionInit = JsFuture::from(peer.create_offer())
            .await
            .map_err(ConnectError::CreateOffer)?
            .into();

        JsFuture::from(peer.set_local_description(&offer))
            .await
            .map_err(ConnectError::SetLocalDescription)?;

        info!("Requesting WebRTC session...");
        let reply: JsValue = request_session(&config.address, &offer).await?;
        info!("Reply: {:?}", reply);

        let (answer, candidate) = (
            Reflect::get(&reply, &JsValue::from_str("answer"))
                .map_err(ConnectError::ResponseJson)?,
            Reflect::get(&reply, &JsValue::from_str("candidate"))
                .map_err(ConnectError::ResponseJson)?,
        );

        JsFuture::from(peer.set_remote_description(&answer.into()))
            .await
            .map_err(ConnectError::SetRemoteDescription)?;

        JsFuture::from(peer.add_ice_candidate_with_opt_rtc_ice_candidate(Some(&candidate.into())))
            .await
            .map_err(ConnectError::AddIceCandidate)?;

        Ok(Client {
            status,
            _on_open: on_open,
            _on_close: on_close,
            _on_error: on_error,
        })
    }

    pub fn status(&self) -> Status {
        self.status.get()
    }
}

pub fn on_open(status: Rc<Cell<Status>>) {
    info!("Connection has been established");

    status.set(Status::Open);
}

pub fn on_close(status: Rc<Cell<Status>>) {
    info!("Connection has been closed");

    status.set(Status::Closed);
}

pub fn on_error(status: Rc<Cell<Status>>, error: &ErrorEvent) {
    warn!("Connection error: {:?}", error);

    status.set(Status::Error);
}

fn new_rtc_peer_connection(config: &Config) -> Result<RtcPeerConnection, ConnectError> {
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

    let mut rtc_configuration = RtcConfiguration::new();
    rtc_configuration.ice_servers(&ice_servers);

    RtcPeerConnection::new_with_configuration(&rtc_configuration)
        .map_err(ConnectError::NewRtcPeerConnection)
}

fn create_data_channel(peer: &RtcPeerConnection) -> RtcDataChannel {
    let mut data_channel_init: RtcDataChannelInit = RtcDataChannelInit::new();
    data_channel_init.ordered(false).max_retransmits(0);

    peer.create_data_channel_with_data_channel_dict("webudp", &data_channel_init)
}

async fn request_session(
    address: &str,
    offer: &RtcSessionDescriptionInit,
) -> Result<JsValue, ConnectError> {
    let mut opts = web_sys::RequestInit::new();
    opts.method("POST");
    opts.mode(web_sys::RequestMode::SameOrigin);
    opts.body(Some(
        &Reflect::get(&offer, &JsValue::from_str("sdp")).unwrap(),
    ));

    let request = web_sys::Request::new_with_str_and_init(&address, &opts)
        .map_err(ConnectError::NewRequest)?;

    let window = web_sys::window().unwrap();
    let response_value: JsValue = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(ConnectError::Fetch)?;
    assert!(response_value.is_instance_of::<web_sys::Response>());
    let response: web_sys::Response = response_value.dyn_into().unwrap();

    if response.status() != 200 {
        return Err(ConnectError::ResponseStatus(response.status()));
    }

    JsFuture::from(response.json().map_err(ConnectError::ResponseJson)?)
        .await
        .map_err(ConnectError::ResponseJson)
}
