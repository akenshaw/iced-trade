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
pub struct StreamWrapper {
    pub stream: String,
    pub data: serde_json::Value,
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
#[derive(Deserialize, Debug, Clone)]
pub struct Kline {
    #[serde(rename = "t")]
    pub time: u64,
    #[serde(with = "string_to_f32", rename = "o")]
    pub open: f32,
    #[serde(with = "string_to_f32", rename = "h")]
    pub high: f32,
    #[serde(with = "string_to_f32", rename = "l")]
    pub low: f32,
    #[serde(with = "string_to_f32", rename = "c")]
    pub close: f32,
    #[serde(with = "string_to_f32", rename = "v")]
    pub volume: f32,
    #[serde(with = "string_to_f32", rename = "V")]
    pub taker_buy_base_asset_volume: f32,
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
                        let websocket_server = format!("wss://fstream.binance.com/stream?streams={}@aggTrade/{}@kline_1m", selected_ticker.to_lowercase(), selected_ticker.to_lowercase());

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
                                        let parsed_message: Result<StreamWrapper, _> = serde_json::from_str(&message);
                                        match parsed_message {
                                            Ok(mut wrapper) => {
                                                if wrapper.stream.contains("aggTrade") {
                                                    let trade: Result<Trade, _> = serde_json::from_str(&wrapper.data.to_string());
                                                    match trade {
                                                        Ok(trade) => {
                                                            trades_buffer.push(trade);
                                                        },
                                                        Err(e) => {
                                                            dbg!(e);
                                                        }
                                                    }
                                                } else if wrapper.stream.contains("kline") {
                                                    if let Some(kline) = wrapper.data.get_mut("k") {
                                                        let kline: Result<Kline, _> = serde_json::from_value(kline.take());
                                                        match kline {
                                                            Ok(kline) => {
                                                                let _ = output.send(Event::KlineReceived(kline)).await;
                                                            },
                                                            Err(e) => {
                                                                dbg!(e);
                                                            }
                                                        }
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                dbg!(e);
                                                dbg!(message); 
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
                                    let _ = output.send(Event::TradeReceived(std::mem::take(&mut trades_buffer))).await;
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
    TradeReceived(Vec<Trade>),
    KlineReceived(Kline),
}

#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<String>);

#[derive(Deserialize, Debug, Clone)]
struct FetchedKlines (
    u64,
    #[serde(with = "string_to_f32")] f32,
    #[serde(with = "string_to_f32")] f32,
    #[serde(with = "string_to_f32")] f32,
    #[serde(with = "string_to_f32")] f32,
    #[serde(with = "string_to_f32")] f32,
    u64,
    String,
    u32,
    #[serde(with = "string_to_f32")] f32,
    String,
    String,
);
impl From<FetchedKlines> for Kline {
    fn from(fetched: FetchedKlines) -> Self {
        Self {
            time: fetched.0,
            open: fetched.1,
            high: fetched.2,
            low: fetched.3,
            close: fetched.4,
            volume: fetched.5,
            taker_buy_base_asset_volume: fetched.9,
        }
    }
}
pub async fn fetch_klines(ticker: String) -> Result<Vec<Kline>, reqwest::Error> {
    let url = format!("https://fapi.binance.com/fapi/v1/klines?symbol={}&interval=1m&limit=30", ticker.to_lowercase());
    let response = reqwest::get(&url).await?;
    let value: serde_json::Value = response.json().await?;
    let fetched_klines: Result<Vec<FetchedKlines>, _> = serde_json::from_value(value);
    let klines: Vec<Kline> = fetched_klines.unwrap().into_iter().map(Kline::from).collect();
    Ok(klines)
}