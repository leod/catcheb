//! Unreliable WebRTC client.
//!
//! This is based on the `echo_server.html` example from `webrtc-unreliable`,
//! but translated from JavaScript into Rust.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

use log::{info, warn};

use js_sys::{Reflect, JSON};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, ErrorEvent, Event, MessageEvent, RtcConfiguration, RtcDataChannel, RtcDataChannelInit,
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

/// In the callback for received messages, Firefox gives us a `Blob`, while
/// Chrome directly gives us the `ArrayBuffer` instead. I'm not sure why.
/// AFAIK, a `Blob` can be extracted only using `await`, so we have this
/// enum for storing the different kinds of values.
pub enum ReceivedData {
    Blob(Blob),
    ArrayBuffer(js_sys::ArrayBuffer),
}

pub struct Data {
    on_message: Box<dyn FnMut(&Data, &[u8])>,
    peer: RtcPeerConnection,
    channel: RtcDataChannel,
    status: Status,
    received: VecDeque<ReceivedData>,
}

pub struct Client {
    data: Rc<RefCell<Data>>,
    _on_open: Closure<dyn FnMut(&Event)>,
    _on_close: Closure<dyn FnMut(&Event)>,
    _on_error: Closure<dyn FnMut(&ErrorEvent)>,
    _on_message: Closure<dyn FnMut(&MessageEvent)>,
}

impl Client {
    pub async fn connect(
        config: Config,
        on_message: Box<dyn FnMut(&Data, &[u8])>,
    ) -> Result<Self, ConnectError> {
        info!("Establishing WebRTC connection");

        let peer: RtcPeerConnection = new_rtc_peer_connection(&config)?;
        let channel: RtcDataChannel = create_data_channel(&peer);

        let data = Rc::new(RefCell::new(Data {
            on_message,
            peer: peer.clone(),
            channel,
            status: Status::Connecting,
            received: VecDeque::new(),
        }));

        let on_open = Closure::wrap(Box::new({
            let data = data.clone();
            move |_: &Event| data.borrow_mut().on_open()
        }) as Box<dyn FnMut(&Event)>);
        // TODO: We'll also want to handle close events caused by the peer
        let on_close = Closure::wrap(Box::new({
            let data = data.clone();
            move |_: &Event| data.borrow_mut().on_close()
        }) as Box<dyn FnMut(&Event)>);
        let on_error = Closure::wrap(Box::new({
            let data = data.clone();
            move |event: &ErrorEvent| data.borrow_mut().on_error(event)
        }) as Box<dyn FnMut(&ErrorEvent)>);
        let on_message = Closure::wrap(Box::new({
            let data = data.clone();
            move |event: &MessageEvent| data.borrow_mut().on_message(event)
        }) as Box<dyn FnMut(&MessageEvent)>);

        {
            let data = data.borrow_mut();
            data.channel
                .set_onopen(Some(on_open.as_ref().unchecked_ref()));
            data.channel
                .set_onclose(Some(on_close.as_ref().unchecked_ref()));
            data.channel
                .set_onerror(Some(on_error.as_ref().unchecked_ref()));
            data.channel
                .set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        }

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
            data,
            _on_open: on_open,
            _on_close: on_close,
            _on_error: on_error,
            _on_message: on_message,
        })
    }

    pub async fn take_message(&mut self) -> Option<comn::ServerMessage> {
        while let Some(received_data) = {
            // Note: It is important to release the borrow before entering the
            // loop, so that we do not hold a borrow when doing await.
            let mut data = self.data.borrow_mut();
            data.received.pop_front()
        } {
            let abuf = match received_data {
                ReceivedData::Blob(blob) => JsFuture::from(blob.array_buffer())
                    .await
                    .unwrap()
                    .dyn_into::<js_sys::ArrayBuffer>()
                    .unwrap(),
                ReceivedData::ArrayBuffer(abuf) => abuf,
            };
            let array = js_sys::Uint8Array::new(&abuf);

            if let Some(message) = comn::ServerMessage::deserialize(&array.to_vec()) {
                return Some(message);
            } else {
                warn!("Failed to deserialize message, ignoring");
                continue;
            }
        }

        None
    }

    pub fn send(&self, data: &[u8]) -> Result<(), JsValue> {
        self.data.borrow().channel.send_with_u8_array(data)
    }

    pub fn status(&self) -> Status {
        self.data.borrow().status
    }
}

impl Data {
    pub fn on_open(&mut self) {
        info!("Connection has been established");

        self.status = Status::Open;
    }

    pub fn on_close(&mut self) {
        info!("Connection has been closed");

        self.status = Status::Closed;
    }

    pub fn on_error(&mut self, error: &ErrorEvent) {
        warn!("Connection error: {:?}", error);

        self.status = Status::Error;
    }

    pub fn on_message(&mut self, message: &MessageEvent) {
        let data = if message.data().is_instance_of::<Blob>() {
            ReceivedData::Blob(message.data().dyn_into::<Blob>().unwrap())
        } else if message.data().is_instance_of::<js_sys::ArrayBuffer>() {
            ReceivedData::ArrayBuffer(message.data().dyn_into::<js_sys::ArrayBuffer>().unwrap())
        } else {
            warn!(
                "Received data {:?}, don't know how to handle",
                message.data()
            );
            return;
        };

        self.received.push_back(data);
    }
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
