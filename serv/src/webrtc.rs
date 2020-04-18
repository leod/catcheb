use tokio::sync::mpsc;

pub type RecvMessageTx = mpsc::UnboundedSender<Vec<u8>>;
pub type RecvMessageRx = mpsc::UnboundedReceiver<Vec<u8>>;
pub type SendMessageTx = mpsc::UnboundedSender<Vec<u8>>;
pub type SendMessageRx = mpsc::UnboundedReceiver<Vec<u8>>;
