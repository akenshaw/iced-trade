use iced::futures;  
use iced::subscription::{self, Subscription};
use serde::{de, Deserialize, Deserializer};
use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::StreamExt;

use async_tungstenite::tungstenite;
use serde_json::Value;
use crate::{Ticker, Timeframe};

use tokio::time::{interval, Duration};
use futures::FutureExt;
use std::sync::{Arc, RwLock, Mutex};
use std::collections::{BTreeMap, HashMap};

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
    DepthReceived(i64, Depth, Vec<Trade>),
    KlineReceived(Kline, Timeframe),
}

#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<String>);

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Depth {
    pub last_update_id: i64,
    #[serde(rename = "T")]
    pub time: i64,
    #[serde(rename = "b")]
    pub bids: BTreeMap<i32, f32>,
    #[serde(rename = "a")]
    pub asks: BTreeMap<i32, f32>,
}

impl Depth {
    pub fn new() -> Self {
        Self {
            last_update_id: 0,
            time: 0,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn fetched(&mut self, new_depth: Depth) {
        self.last_update_id = new_depth.last_update_id;        
        self.time = new_depth.time;

        self.bids = new_depth.bids;
        self.asks = new_depth.asks;
    }

    pub fn update_levels(&mut self, new_depth: Depth) {
        self.last_update_id = new_depth.last_update_id;
        self.time = new_depth.time;

        for order in &new_depth.bids {
            if *order.1 == 0.0 {
                self.bids.remove(order.0);
            } else {
                self.bids.insert(*order.0, *order.1);
            }
        }
        for order in &new_depth.asks {
            if *order.1 == 0.0 {
                self.asks.remove(order.0);
            } else {
                self.asks.insert(*order.0, *order.1);
            }
        }
    }

    pub fn get_fetch_id(&self) -> i64 {
        self.last_update_id
    }
}

fn convert_to_btreemap(array: &Value) -> BTreeMap<i32, f32> {
    array.as_array().unwrap().iter().map(|v| {
        let v_array = v.as_array().unwrap();
        let key = (v_array[0].as_str().unwrap().parse::<f32>().unwrap() * 100.0) as i32;
        let value = v_array[1].as_str().unwrap().parse::<f32>().unwrap();
        (key, value)
    }).collect()
}

pub fn connect_market_stream(selected_ticker: Ticker) -> Subscription<Event> {
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

            let stream_1 = format!("{symbol_str}@aggTrade");
            let stream_2 = format!("{symbol_str}@depth@100ms");

            let mut orderbook: Depth = Depth::new();

            let mut already_fetching: bool = false;

            let mut prev_id: i64 = 0;

            loop {
                match &mut state {
                    State::Disconnected => {        
                        let websocket_server = format!("wss://fstream.binance.com/stream?streams={stream_1}/{stream_2}");

                        if let Ok((websocket, _)) = async_tungstenite::tokio::connect_async(
                            websocket_server,
                        )
                        .await {
                            let (tx, rx) = tokio::sync::oneshot::channel();
                                                
                            tokio::spawn(async move {
                                let fetched_depth = fetch_depth(selected_ticker).await;

                                let depth: Depth = match fetched_depth {
                                    Ok(depth) => {
                                        Depth {
                                            last_update_id: depth.update_id,
                                            time: depth.time,
                                            bids: depth.bids.iter().map(|(price, qty)| (*price, *qty)).collect(),
                                            asks: depth.asks.iter().map(|(price, qty)| (*price, *qty)).collect(),
                                        }
                                    },
                                    Err(_) => return,
                                };

                                let _ = tx.send(depth);
                            });
                            match rx.await {
                                Ok(depth) => {
                                    orderbook.fetched(depth);
                                    state = State::Connected(websocket);
                                },
                                Err(_) => orderbook.fetched(Depth::default()),
                            }
                            
                        } else {
                            tokio::time::sleep(tokio::time::Duration::from_secs(1))
                           .await;
                           let _ = output.send(Event::Disconnected).await;
                        }
                    }
                    State::Connected(websocket) => {
                        let mut fused_websocket = websocket.by_ref().fuse();

                        futures::select! {
                            received = fused_websocket.select_next_some() => {
                                match received {
                                    Ok(tungstenite::Message::Text(message)) => {
                                        let stream: Stream = serde_json::from_str(&message).unwrap_or(Stream { stream: String::new() });
                                        
                                        if stream.stream == stream_1 {
                                            let agg_trade: AggTrade = serde_json::from_str(&message).unwrap();
                                            trades_buffer.push(agg_trade.data);
                                            
                                        } else if stream.stream == stream_2 {
                                            if already_fetching {
                                                println!("Already fetching...\n");

                                                continue;
                                            }

                                            let depth_update: Value = serde_json::from_str(&message).unwrap();
                                            let depth_data = depth_update.get("data").unwrap();

                                            let first_update_id = depth_data.get("U").unwrap().as_i64().unwrap();
                                            let final_update_id = depth_data.get("u").unwrap().as_i64().unwrap();

                                            let last_final_update_id = depth_data.get("pu").unwrap().as_i64().unwrap();

                                            let last_update_id = orderbook.get_fetch_id();

                                            if (final_update_id <= last_update_id) || last_update_id == 0 {
                                                continue;
                                            }

                                            if prev_id == 0 && (first_update_id > last_update_id + 1) || (last_update_id + 1 > final_update_id) {
                                                println!("Out of sync on first event...\nU: {first_update_id}, last_id: {last_update_id}, u: {final_update_id}, pu: {last_final_update_id}\n");

                                                let (tx, rx) = tokio::sync::oneshot::channel();
                                                already_fetching = true;

                                                tokio::spawn(async move {
                                                    let fetched_depth = fetch_depth(selected_ticker).await;

                                                    let depth: Depth = match fetched_depth {
                                                        Ok(depth) => {
                                                            Depth {
                                                                last_update_id: depth.update_id,
                                                                time: depth.time,
                                                                bids: depth.bids.iter().map(|(price, qty)| (*price, *qty)).collect(),
                                                                asks: depth.asks.iter().map(|(price, qty)| (*price, *qty)).collect(),
                                                            }
                                                        },
                                                        Err(_) => return,
                                                    };

                                                    let _ = tx.send(depth);
                                                });
                                                match rx.await {
                                                    Ok(depth) => {
                                                        orderbook.fetched(depth)
                                                    },
                                                    Err(_) => orderbook.fetched(Depth::default()),
                                                }
                                                already_fetching = false;
                                            }
                                    
                                            if (prev_id == 0) || (prev_id == last_final_update_id) {
                                                let time: i64 = depth_data.get("T").unwrap().as_i64().unwrap();
                                                let bids: BTreeMap<i32, f32> = convert_to_btreemap(depth_data.get("b").unwrap());
                                                let asks: BTreeMap<i32, f32> = convert_to_btreemap(depth_data.get("a").unwrap());

                                                let depth = Depth { last_update_id: final_update_id, time, bids, asks };

                                                orderbook.update_levels(depth);

                                                let _ = output.send(
                                                    Event::DepthReceived(
                                                        time, 
                                                        orderbook.clone(),
                                                        std::mem::take(&mut trades_buffer)
                                                    )
                                                ).await;

                                                prev_id = final_update_id;
                                            } else {
                                                println!("Out of sync...\n");
                                            }

                                        } else {
                                            dbg!(stream.stream);
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

pub fn connect_kline_stream(vec: Vec<(Ticker, Timeframe)>) -> Subscription<Event> {
    struct Connect;

    subscription::channel(
        std::any::TypeId::of::<Connect>(),
        100,
        move |mut output| async move {
            let mut state = State::Disconnected;    

            let stream_str = vec.iter().map(|(ticker, timeframe)| {
                let symbol_str = match ticker {
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
                format!("{symbol_str}@kline_{timeframe_str}")
            }).collect::<Vec<String>>().join("/");
 
            loop {
                match &mut state {
                    State::Disconnected => {
                        let websocket_server = format!("wss://fstream.binance.com/stream?streams={stream_str}");
                        
                        if let Ok((websocket, _)) = async_tungstenite::tokio::connect_async(
                            websocket_server,
                        )
                        .await {
                           state = State::Connected(websocket);
                        } else {
                            tokio::time::sleep(tokio::time::Duration::from_secs(1))
                           .await;
                           let _ = output.send(Event::Disconnected).await;
                        }
                    }
                    State::Connected(websocket) => {
                        let mut fused_websocket = websocket.by_ref().fuse();

                        futures::select! {
                            received = fused_websocket.select_next_some() => {
                                match received {
                                    Ok(tungstenite::Message::Text(message)) => {
                                        match serde_json::from_str::<serde_json::Value>(&message) {
                                            Ok(data) => {
                                                match (data.get("data"), data["data"]["k"]["i"].as_str(), data["data"]["k"].as_object()) {
                                                    (Some(inner_data), Some(interval), Some(kline_obj)) if inner_data["e"].as_str() == Some("kline") => {
                                                        let kline = Kline {
                                                            time: kline_obj["t"].as_u64().unwrap_or_default(),
                                                            open: kline_obj["o"].as_str().unwrap_or_default().parse::<f32>().unwrap_or_default(),
                                                            high: kline_obj["h"].as_str().unwrap_or_default().parse::<f32>().unwrap_or_default(),
                                                            low: kline_obj["l"].as_str().unwrap_or_default().parse::<f32>().unwrap_or_default(),
                                                            close: kline_obj["c"].as_str().unwrap_or_default().parse::<f32>().unwrap_or_default(),
                                                            volume: kline_obj["v"].as_str().unwrap_or_default().parse::<f32>().unwrap_or_default(),
                                                            taker_buy_base_asset_volume: kline_obj["V"].as_str().unwrap_or_default().parse::<f32>().unwrap_or_default(),
                                                        };
                                                
                                                        if let Some(timeframe) = vec.iter().find(|(_, tf)| tf.to_string() == interval) {
                                                            let _ = output.send(Event::KlineReceived(kline, timeframe.1)).await;
                                                        }
                                                    },
                                                    _ => continue,
                                                }                                                
                                            },
                                            Err(_) => continue,
                                        }
                                    },
                                    Err(_) => {
                                        let _ = output.send(Event::Disconnected).await;
                                        state = State::Disconnected;
                                    },
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

mod string_to_f32 {
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = <&str>::deserialize(deserializer)?;
        s.parse::<f32>().map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize)]
struct Stream {
    stream: String,
}
#[derive(Deserialize, Debug)]
struct AggTrade {
    data: Trade,
}
#[derive(Deserialize, Debug)]
struct DepthUpdate {
    data: Depth,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Trade {
    #[serde(rename = "T")]
    pub time: i64,
    #[serde(rename = "m")]
    pub is_sell: bool,
    #[serde(with = "string_to_f32", rename = "p")]
    pub price: f32,
    #[serde(with = "string_to_f32", rename = "q")]
    pub qty: f32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FetchedDepth {
    #[serde(rename = "lastUpdateId")]
    pub update_id: i64,
    #[serde(rename = "T")]
    pub time: i64,
    #[serde(rename = "bids")]
    pub bids: BTreeMap<i32, f32>,
    #[serde(rename = "asks")]
    pub asks: BTreeMap<i32, f32>,
}

#[derive(Deserialize, Debug, Clone, Copy)]
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

pub async fn fetch_klines(ticker: Ticker, timeframe: Timeframe) -> Result<Vec<Kline>, reqwest::Error> {
    let symbol_str = match ticker {
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

    let url = format!("https://fapi.binance.com/fapi/v1/klines?symbol={symbol_str}&interval={timeframe_str}&limit=720");

    let response = reqwest::get(&url).await?;
    let text = response.text().await?;
    let fetched_klines: Result<Vec<FetchedKlines>, _> = serde_json::from_str(&text);
    let klines: Vec<Kline> = fetched_klines.unwrap().into_iter().map(Kline::from).collect();

    Ok(klines)
}

pub async fn fetch_depth(ticker: Ticker) -> Result<FetchedDepth, reqwest::Error> {
    let symbol_str = match ticker {
        Ticker::BTCUSDT => "btcusdt",
        Ticker::ETHUSDT => "ethusdt",
        Ticker::SOLUSDT => "solusdt",
        Ticker::LTCUSDT => "ltcusdt",
    };

    let url = format!("https://fapi.binance.com/fapi/v1/depth?symbol={symbol_str}&limit=100");

    let response = reqwest::get(&url).await?;
    let text = response.text().await?;
    let depth: Value = serde_json::from_str(&text).unwrap();

    let depth = FetchedDepth {
        update_id: depth["lastUpdateId"].as_i64().unwrap(),
        time: depth["T"].as_i64().unwrap(),
        bids: convert_to_btreemap(&depth["bids"]),
        asks: convert_to_btreemap(&depth["asks"]),
    };

    dbg!(&depth.update_id);

    Ok(depth)
}