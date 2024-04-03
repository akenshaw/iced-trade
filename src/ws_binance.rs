use iced::futures;  
use iced::subscription::{self, Subscription};
use serde::Deserialize;

mod string_to_f32 {
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<f32>().map_err(serde::de::Error::custom)
    }
}

use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use futures::FutureExt;

use async_tungstenite::tungstenite;

#[derive(Deserialize, Debug, Clone)]
pub struct TradeWrapper {
    pub stream: String,
    pub data: Trade,
}
#[derive(Deserialize, Debug, Clone)]
pub struct Trade {
    #[serde(rename = "T")]
    pub time: u64,
    #[serde(rename = "m")]
    pub is_sell: bool,
    #[serde(with = "string_to_f32", rename = "p")]
    pub price: f32,
    #[serde(with = "string_to_f32", rename = "q")]
    pub qty: f32,
}
pub fn connect(selected_ticker: String) -> Subscription<Event> {
    struct Connect;

    subscription::channel(
        std::any::TypeId::of::<Connect>(),
        100,
        |mut output| async move {
            let mut state = State::Disconnected;     
            let mut trades_buffer = Vec::new(); 
            let buffer_flush_interval = tokio::time::Duration::from_millis(1000 / 30);
 
            loop {
                match &mut state {
                    State::Disconnected => {
                        let websocket_server = format!("wss://fstream.binance.com/stream?streams={}@aggTrade", selected_ticker.to_lowercase());

                        match async_tungstenite::tokio::connect_async(
                            websocket_server,
                        )
                        .await
                        {
                            Ok((websocket, _)) => {
                                state = State::Connected(websocket);
                            }
                            Err(_) => {
                                tokio::time::sleep(
                                    tokio::time::Duration::from_secs(1),
                                )
                                .await;

                                let _ = output.send(Event::Disconnected).await;
                            }
                        }
                    }
                    State::Connected(websocket) => {
                        let mut fused_websocket = websocket.by_ref().fuse();

                        futures::select! {
                            received = fused_websocket.select_next_some() => {
                                match received {
                                    Ok(tungstenite::Message::Text(message)) => {
                                        let parsed_message: Result<TradeWrapper, _> = serde_json::from_str(&message);
                                        match parsed_message {
                                            Ok(message) => {
                                                trades_buffer.push(message);
                                            },
                                            Err(e) => {
                                                dbg!(e);
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        let _ = output.send(Event::Disconnected).await;
                                        state = State::Disconnected;
                                    }
                                    Ok(_) => continue,
                                }
                            }
                            _ = tokio::time::sleep(buffer_flush_interval).fuse() => {
                                if !trades_buffer.is_empty() {
                                    let _ = output.send(Event::MessageReceived(std::mem::take(&mut trades_buffer))).await;
                                }
                            }
                        }
                    }
                }
            }
        },
    )
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum State {
    Disconnected,
    Connected(
        async_tungstenite::WebSocketStream<
            async_tungstenite::tokio::ConnectStream,
        >,
    ),
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(Connection),
    Disconnected,
    MessageReceived(Vec<TradeWrapper>),
}

#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<String>);
