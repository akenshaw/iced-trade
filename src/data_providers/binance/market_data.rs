use iced::futures;  
use iced::subscription::{self, Subscription};
use serde::Deserialize;
use serde_json::json;
use hmac::{Hmac, Mac};

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

use async_tungstenite::tungstenite;

use crate::{Ticker, Timeframe};

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
#[derive(Debug, Clone)]
pub struct Depth {
    pub bids: Vec<(f32, f32)>,
    pub asks: Vec<(f32, f32)>,
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

pub fn connect_market_stream(selected_ticker: Ticker, timeframe: Timeframe) -> Subscription<Event> {
    struct Connect;

    subscription::channel(
        std::any::TypeId::of::<Connect>(),
        100,
        move |mut output| async move {
            let mut state = State::Disconnected;     
            let mut trades_buffer = Vec::new(); 

            let symbol_str = match selected_ticker {
                Ticker::BTCUSDT => "btcusdt",
                Ticker::ETHUSDT => "ethusdt",
                Ticker::SOLUSDT => "solusdt",
                Ticker::LTCUSDT => "ltcusdt",
            };
            let timeframe_str = match timeframe {
                Timeframe::M1 => "1m",
                Timeframe::M3 => "3m",
                Timeframe::M5 => "5m",
                Timeframe::M15 => "15m",
                Timeframe::M30 => "30m",
            };
 
            loop {
                match &mut state {
                    State::Disconnected => {
                        let websocket_server = format!("wss://fstream.binance.com/stream?streams={}@aggTrade/{}@depth20@100ms/{}@kline_{}", symbol_str, symbol_str, symbol_str, timeframe_str);
                        
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
                                        let parsed_message: Result<serde_json::Value, _> = serde_json::from_str(&message);
                                        match parsed_message {
                                            Ok(data) => {
                                                if let Some(inner_data) = data.get("data") {
                                                    if let Some(event_type) = inner_data["e"].as_str() {
                                                        match event_type {
                                                            "aggTrade" => {
                                                                let trade: Result<Trade, _> = serde_json::from_value(data["data"].clone());
                                                                match trade {
                                                                    Ok(trade) => {
                                                                        trades_buffer.push(trade);
                                                                    },
                                                                    Err(e) => {
                                                                        dbg!(e);
                                                                    }
                                                                }
                                                            },
                                                            "depthUpdate" => {
                                                                let update_time = data["data"]["T"].as_u64().unwrap();

                                                                if let Some(bids_data) = data["data"]["b"].as_array() {
                                                                    let bids: Vec<(f32, f32)> = bids_data.iter().map(|bid| {
                                                                        let price = bid[0].as_str().unwrap().parse().unwrap();
                                                                        let qty = bid[1].as_str().unwrap().parse().unwrap();
                                                                        (price, qty)
                                                                    }).collect();

                                                                    if let Some(asks_data) = data["data"]["a"].as_array() {
                                                                        let asks: Vec<(f32, f32)> = asks_data.iter().map(|ask| {
                                                                            let price = ask[0].as_str().unwrap().parse().unwrap();
                                                                            let qty = ask[1].as_str().unwrap().parse().unwrap();
                                                                            (price, qty)
                                                                        }).collect();

                                                                        let _ = output.send(Event::DepthReceived(update_time, bids, asks, std::mem::take(&mut trades_buffer))).await;
                                                                    }
                                                                }
                                                            },
                                                            "kline" => {
                                                                if let Some(kline) = data["data"]["k"].as_object() {
                                                                    let kline: Result<Kline, _> = serde_json::from_value(json!(kline));
                                                                    match kline {
                                                                        Ok(kline) => {
                                                                            let _ = output.send(Event::KlineReceived(kline)).await;
                                                                        },
                                                                        Err(e) => {
                                                                            dbg!(e);
                                                                        }
                                                                    }
                                                                }
                                                            },
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                dbg!(e, message);
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
    DepthReceived(u64, Vec<(f32, f32)>, Vec<(f32, f32)>, Vec<Trade>),
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
pub async fn fetch_klines(ticker: String, timeframe: String) -> Result<Vec<Kline>, reqwest::Error> {
    let url = format!("https://fapi.binance.com/fapi/v1/klines?symbol={}&interval={}&limit=180", ticker.to_lowercase(), timeframe);
    let response = reqwest::get(&url).await?;
    let value: serde_json::Value = response.json().await?;
    let fetched_klines: Result<Vec<FetchedKlines>, _> = serde_json::from_value(value);
    let klines: Vec<Kline> = fetched_klines.unwrap().into_iter().map(Kline::from).collect();
    Ok(klines)
}