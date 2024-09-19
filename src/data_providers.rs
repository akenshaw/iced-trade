use serde::{Deserialize, Serialize};

pub mod binance;
pub mod bybit;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum StreamType {
    Kline {
        exchange: Exchange,
        ticker: Ticker,
        timeframe: Timeframe,
    },
    DepthAndTrades {
        exchange: Exchange,
        ticker: Ticker,
    },
    None,
}

// data types
#[derive(Debug, Clone, Copy, Default)]
pub struct Order {
    pub price: f32,
    pub qty: f32,
}
#[derive(Debug, Clone, Default)]
pub struct Depth {
    pub time: i64,
    pub bids: Vec<Order>,
    pub asks: Vec<Order>,
}

#[derive(Debug, Clone, Default)]
pub struct LocalDepthCache {
    pub last_update_id: i64,
    pub time: i64,
    pub bids: Vec<Order>,
    pub asks: Vec<Order>,
}

impl LocalDepthCache {
    pub fn new() -> Self {
        Self {
            last_update_id: 0,
            time: 0,
            bids: Vec::new(),
            asks: Vec::new(),
        }
    }

    pub fn fetched(&mut self, new_depth: LocalDepthCache) {
        self.last_update_id = new_depth.last_update_id;        
        self.time = new_depth.time;

        self.bids = new_depth.bids;
        self.asks = new_depth.asks;
    }

    pub fn update_depth_cache(&mut self, new_depth: LocalDepthCache) {
        self.last_update_id = new_depth.last_update_id;
        self.time = new_depth.time;

        for order in new_depth.bids.iter() {
            if order.qty == 0.0 {
                self.bids.retain(|x| x.price != order.price);
            } else if let Some(existing_order) = self.bids.iter_mut().find(|x| x.price == order.price) {
                existing_order.qty = order.qty;
            } else {
                self.bids.push(*order);
            }
        }
        for order in new_depth.asks.iter() {
            if order.qty == 0.0 {
                self.asks.retain(|x| x.price != order.price);
            } else if let Some(existing_order) = self.asks.iter_mut().find(|x| x.price == order.price) {
                existing_order.qty = order.qty;
            } else {
                self.asks.push(*order);
            }
        }
    }

    pub fn get_depth(&self) -> Depth {
        Depth {
            time: self.time,
            bids: self.bids.clone(),
            asks: self.asks.clone(),
        }
    }

    pub fn get_fetch_id(&self) -> i64 {
        self.last_update_id
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Trade {
    pub time: i64,
    pub is_sell: bool,
    pub price: f32,
    pub qty: f32,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Kline {
    pub time: u64,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: (f32, f32),
}

#[derive(Default, Debug, Clone, Copy)]
pub struct FeedLatency {
    pub time: i64,
    pub depth_latency: i64,
    pub trade_latency: Option<i64>,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct TickMultiplier(pub u16);

impl std::fmt::Display for TickMultiplier {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}x", self.0)
    }
}

impl TickMultiplier {
    pub fn multiply_with_min_tick_size(&self, min_tick_size: f32) -> f32 {
        self.0 as f32 * min_tick_size
    }
}

// connection types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Exchange {
    BinanceFutures,
    BybitLinear,
}

impl std::fmt::Display for Exchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Exchange::BinanceFutures => "Binance Futures",
                Exchange::BybitLinear => "Bybit Linear",
            }
        )
    }
}
impl Exchange {
    pub const ALL: [Exchange; 2] = [Exchange::BinanceFutures, Exchange::BybitLinear];
}

impl std::fmt::Display for Ticker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Ticker::BTCUSDT => "BTCUSDT",
                Ticker::ETHUSDT => "ETHUSDT",
                Ticker::SOLUSDT => "SOLUSDT",
                Ticker::LTCUSDT => "LTCUSDT",
            }
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Ticker {
    BTCUSDT,
    ETHUSDT,
    SOLUSDT,
    LTCUSDT,
}
impl Ticker {
    pub const ALL: [Ticker; 4] = [Ticker::BTCUSDT, Ticker::ETHUSDT, Ticker::SOLUSDT, Ticker::LTCUSDT];
}

impl Ticker {
    /// Returns the string representation of the ticker in lowercase
    /// 
    /// e.g. BTCUSDT -> "btcusdt"
    pub fn get_string(&self) -> String {
        match self {
            Ticker::BTCUSDT => "btcusdt".to_string(),
            Ticker::ETHUSDT => "ethusdt".to_string(),
            Ticker::SOLUSDT => "solusdt".to_string(),
            Ticker::LTCUSDT => "ltcusdt".to_string(),
        }
    }
}

impl std::fmt::Display for Timeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Timeframe::M1 => "1m",
                Timeframe::M3 => "3m",
                Timeframe::M5 => "5m",
                Timeframe::M15 => "15m",
                Timeframe::M30 => "30m",
            }
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Timeframe {
    M1,
    M3,
    M5,
    M15,
    M30,
}
impl Timeframe {
    pub const ALL: [Timeframe; 5] = [Timeframe::M1, Timeframe::M3, Timeframe::M5, Timeframe::M15, Timeframe::M30];

    pub fn to_minutes(&self) -> u16 {
        match self {
            Timeframe::M1 => 1,
            Timeframe::M3 => 3,
            Timeframe::M5 => 5,
            Timeframe::M15 => 15,
            Timeframe::M30 => 30,
        }
    }
}

#[derive(Debug)]
pub enum BinanceWsState {
    Connected(binance::market_data::Connection),
    Disconnected,
}
impl Default for BinanceWsState {
    fn default() -> Self {
        Self::Disconnected
    }
}

#[derive(Debug)]
pub enum BybitWsState {
    Connected(bybit::market_data::Connection),
    Disconnected,
}
impl Default for BybitWsState {
    fn default() -> Self {
        Self::Disconnected
    }
}

pub enum UserWsState {
    Connected(binance::user_data::Connection),
    Disconnected,
}
impl Default for UserWsState {
    fn default() -> Self {
        Self::Disconnected
    }
}

#[derive(Debug, Clone)]
pub enum MarketEvents {
    Binance(binance::market_data::Event),
    Bybit(bybit::market_data::Event),
}

#[derive(thiserror::Error, Debug)]
pub enum StreamError {
    #[error("FetchError: {0}")]
    FetchError(#[from] reqwest::Error),
    #[error("ParseError: {0}")]
    ParseError(String),
    #[error("StreamError: {0}")]
    WebsocketError(String),
    #[error("UnknownError: {0}")]
    UnknownError(String),
}