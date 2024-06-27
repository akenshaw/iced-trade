use std::os::macos::raw::stat;

use iced::futures;  
use iced::subscription::{self, Subscription};
use serde::{de, Deserialize, Deserializer};
use futures::sink::SinkExt;
use futures::stream::StreamExt;

use async_tungstenite::tungstenite;
use serde_json::Value;
use crate::{Ticker, Timeframe};

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
    DepthReceived(i64, LocalDepthCache, Vec<Trade>),
    KlineReceived(Kline, Timeframe),
}

#[derive(Debug, Clone)]
pub struct Connection;

#[derive(Debug, Deserialize, Clone)]
pub struct FetchedDepth {
    #[serde(rename = "b")]
    pub bids: Vec<Order>,
    #[serde(rename = "a")]
    pub asks: Vec<Order>,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Order {
    pub price: f32,
    pub qty: f32,
}
#[derive(Debug, Clone, Default)]
pub struct LocalDepthCache {
    pub time: i64,
    pub bids: Box<[Order]>,
    pub asks: Box<[Order]>,
}
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Depth {
    #[serde(default)]
    pub last_update_id: i64,
    #[serde(rename = "T")]
    pub time: i64,
    #[serde(rename = "b")]
    pub bids: Vec<Order>,
    #[serde(rename = "a")]
    pub asks: Vec<Order>,
}

use std::str::FromStr;
impl<'de> Deserialize<'de> for Order {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value: Vec<String> = Deserialize::deserialize(deserializer)?;
        if value.len() != 2 {
            return Err(serde::de::Error::custom("Expected an array of two strings"));
        }

        let price = f32::from_str(&value[0]).map_err(serde::de::Error::custom)?;
        let qty = f32::from_str(&value[1]).map_err(serde::de::Error::custom)?;

        Ok(Order { price, qty })
    }
}

impl Depth {
    pub fn new() -> Self {
        Self {
            last_update_id: 0,
            time: 0,
            bids: Vec::new(),
            asks: Vec::new(),
        }
    }

    pub fn fetched(&mut self, new_depth: Depth) {
        self.last_update_id = new_depth.last_update_id;        
        self.time = new_depth.time;

        self.bids = new_depth.bids;
        self.asks = new_depth.asks;

        println!("Fetched depth: {:?}", self);
    }

    pub fn update_depth_cache(&mut self, new_bids: &[Order], new_asks: &[Order]) {
        for order in new_bids {
            if order.qty == 0.0 {
                self.bids.retain(|x| x.price != order.price);
            } else {
                if let Some(existing_order) = self.bids.iter_mut().find(|x| x.price == order.price) {
                    existing_order.qty = order.qty;
                } else {
                    self.bids.push(*order);
                }
            }
        }
        for order in new_asks {
            if order.qty == 0.0 {
                self.asks.retain(|x| x.price != order.price);
            } else {
                if let Some(existing_order) = self.asks.iter_mut().find(|x| x.price == order.price) {
                    existing_order.qty = order.qty;
                } else {
                    self.asks.push(*order);
                }
            }
        }
    }

    pub fn update_levels(&mut self, new_depth: Depth) -> (Box<[Order]>, Box<[Order]>) {
        self.last_update_id = new_depth.last_update_id;
        self.time = new_depth.time;

        let mut best_ask_price = f32::MAX;
        let mut best_bid_price = 0.0f32;

        self.bids.iter().for_each(|order| {
            if order.price > best_bid_price {
                best_bid_price = order.price;
            }
        });
        self.asks.iter().for_each(|order| {
            if order.price < best_ask_price {
                best_ask_price = order.price;
            }
        });

        let highest: f32 = best_ask_price * 1.001;
        let lowest: f32 = best_bid_price * 0.999;

        self.update_depth_cache(&new_depth.bids, &new_depth.asks);

        let mut local_bids: Vec<Order> = Vec::new();
        let mut local_asks: Vec<Order> = Vec::new();

        for order in &self.bids {
            if order.price >= lowest {
                local_bids.push(*order);
            }
        }
        for order in &self.asks {
            if order.price <= highest {
                local_asks.push(*order);
            }
        }

        // first sort by price
        local_bids.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap());
        local_asks.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());

        (local_bids.into_boxed_slice(), local_asks.into_boxed_slice())
    }

    pub fn get_fetch_id(&self) -> i64 {
        self.last_update_id
    }
}

pub fn connect_market_stream(selected_ticker: Ticker) -> Subscription<Event> {
    struct Connect;

    subscription::channel(
        std::any::TypeId::of::<Connect>(),
        100,
        move |mut output| async move {
            let mut state: State = State::Disconnected;  

            let mut trades_buffer: Vec<Trade> = Vec::new();    

            let symbol_str = match selected_ticker {
                Ticker::BTCUSDT => "BTCUSDT",
                Ticker::ETHUSDT => "ETHUSDT",
                Ticker::SOLUSDT => "SOLUSDT",
                Ticker::LTCUSDT => "LTCUSDT",
            };

            let stream_1 = format!("publicTrade.{symbol_str}");
            let stream_2 = format!("orderbook.200.{symbol_str}");

            let mut orderbook: Depth = Depth::new();

            let mut already_fetching: bool = false;

            let mut prev_id: i64 = 0;

            loop {
                match &mut state {
                    State::Disconnected => {        
                        let websocket_server = format!("wss://stream.bybit.com/v5/public/linear");

                        println!("Connecting to websocket server...\n");

                        if let Ok((mut websocket, _)) = async_tungstenite::tokio::connect_async(
                            websocket_server,
                        )
                        .await {
                            let subscribe_message = serde_json::json!({
                                "op": "subscribe",
                                "args": [format!("publicTrade.{symbol_str}"), format!("orderbook.200.{symbol_str}")]
                            }).to_string();
    
                            if let Err(e) = websocket.send(tungstenite::Message::Text(subscribe_message)).await {
                                eprintln!("Failed subscribing: {}", e);

                                let _ = output.send(Event::Disconnected).await;

                                continue;
                            } 

                            state = State::Connected(websocket);
                            
                        } else {
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                            let _ = output.send(Event::Disconnected).await;
                        }
                    }
                    State::Connected(websocket) => {
                        let mut fused_websocket = websocket.by_ref().fuse();

                        futures::select! {
                            received = fused_websocket.select_next_some() => {
                                match received {
                                    Ok(tungstenite::Message::Text(message)) => {
                                        match serde_json::from_str::<Stream>(&message) {
                                            Ok(stream) => {
                                                if stream.topic == stream_1 {
                                                    stream.data.as_array().unwrap().iter().for_each(|trade| {
                                                        if let Ok(trade) = serde_json::from_value::<Trade>(trade.clone()) {
                                                            trades_buffer.push(trade);
                                                        } else {
                                                            println!("Failed to deserialize trade: {:?}", trade);
                                                        }
                                                    });

                                                } else if stream.topic == stream_2 {

                                                    if stream.stream_type == "snapshot" {
                                                        let bids = stream.data["b"].as_array().unwrap();
                                                        let asks = stream.data["a"].as_array().unwrap();

                                                        let fetched_depth = Depth {
                                                            last_update_id: stream.time,
                                                            time: stream.time,
                                                            bids: bids.iter().map(|x| serde_json::from_value::<Order>(x.clone()).unwrap()).collect(),
                                                            asks: asks.iter().map(|x| serde_json::from_value::<Order>(x.clone()).unwrap()).collect(),
                                                        };

                                                        orderbook.fetched(fetched_depth);

                                                    } else if stream.stream_type == "delta" {
                                                        let bids = stream.data["b"].as_array().unwrap();
                                                        let asks = stream.data["a"].as_array().unwrap();

                                                        let new_depth = Depth {
                                                            last_update_id: stream.time,
                                                            time: stream.time,
                                                            bids: bids.iter().map(|x| serde_json::from_value::<Order>(x.clone()).unwrap()).collect(),
                                                            asks: asks.iter().map(|x| serde_json::from_value::<Order>(x.clone()).unwrap()).collect(),
                                                        };

                                                        let (local_bids, local_asks) = orderbook.update_levels(new_depth);

                                                        let _ = output.send(Event::DepthReceived(stream.time, LocalDepthCache {
                                                            time: stream.time,
                                                            bids: local_bids,
                                                            asks: local_asks,
                                                        }, std::mem::take(&mut trades_buffer))).await;
                                                    }
                                                }
                                            },
                                            Err(e) => println!("Failed to deserialize message: {}. Error: {}", message, e),
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

#[derive(Deserialize)]
struct Stream {
    topic: String,
    #[serde(rename = "type")]
    stream_type: String,
    #[serde(rename = "ts")]
    time: i64,
    data: Value,
}
 
#[derive(Deserialize, Debug, Clone)]
pub struct Trade {
    #[serde(rename = "T")]
    pub time: i64,
    #[serde(rename = "S", deserialize_with = "deserialize_is_sell")]
    pub is_sell: bool,
    #[serde(with = "string_to_f32", rename = "p")]
    pub price: f32,
    #[serde(with = "string_to_f32", rename = "v")]
    pub qty: f32,
}
fn deserialize_is_sell<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    match s.as_str() {
        "Sell" => Ok(true),
        "Buy" => Ok(false),
        _ => Err(serde::de::Error::custom("Unexpected value for is_sell")),
    }
}
mod string_to_f32 {
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        s.parse::<f32>().map_err(serde::de::Error::custom)
    }
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

    let url = format!("https://fapi.binance.com/fapi/v1/depth?symbol={symbol_str}&limit=500");

    let response = reqwest::get(&url).await?;
    let text = response.text().await?;
    let depth: FetchedDepth = serde_json::from_str(&text).unwrap();

    Ok(depth)
}

pub async fn fetch_ticksize(ticker: Ticker) -> Result<f32, reqwest::Error> {
    let symbol_str = match ticker {
        Ticker::BTCUSDT => "BTCUSDT",
        Ticker::ETHUSDT => "ETHUSDT",
        Ticker::SOLUSDT => "SOLUSDT",
        Ticker::LTCUSDT => "LTCUSDT",
    };

    let url = format!("https://fapi.binance.com/fapi/v1/exchangeInfo");

    let response = reqwest::get(&url).await?;
    let text = response.text().await?;
    let exchange_info: Value = serde_json::from_str(&text).unwrap();

    let symbols = exchange_info["symbols"].as_array().unwrap();

    let symbol = symbols.iter().find(|x| x["symbol"].as_str().unwrap() == symbol_str).unwrap();

    let tick_size = symbol["filters"].as_array().unwrap().iter().find(|x| x["filterType"].as_str().unwrap() == "PRICE_FILTER").unwrap()["tickSize"].as_str().unwrap().parse::<f32>().unwrap();

    Ok(tick_size)
}