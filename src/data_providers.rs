pub mod binance;
pub mod bybit;

#[derive(Debug, Clone, Copy, Default)]
pub struct Order {
    pub price: f32,
    pub qty: f32,
}
#[derive(Debug, Clone, Default)]
pub struct Depth {
    pub time: i64,
    pub bids: Box<[Order]>,
    pub asks: Box<[Order]>,
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

    pub fn update_depth_cache(&mut self, new_bids: &[Order], new_asks: &[Order]) {
        for order in new_bids {
            if order.qty == 0.0 {
                self.bids.retain(|x| x.price != order.price);
            } else if let Some(existing_order) = self.bids.iter_mut().find(|x| x.price == order.price) {
                    existing_order.qty = order.qty;
            } else {
                self.bids.push(*order);
            }
        }
        for order in new_asks {
            if order.qty == 0.0 {
                self.asks.retain(|x| x.price != order.price);
            } else if let Some(existing_order) = self.asks.iter_mut().find(|x| x.price == order.price) {
                existing_order.qty = order.qty;
            } else {
                self.asks.push(*order);
            }
        }
    }

    pub fn update_levels(&mut self, new_depth: LocalDepthCache) -> (Box<[Order]>, Box<[Order]>) {
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

#[derive(Debug, Clone, Copy)]
pub struct Trade {
    pub time: i64,
    pub is_sell: bool,
    pub price: f32,
    pub qty: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Kline {
    pub time: u64,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub taker_buy_base_asset_volume: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct FeedLatency {
    pub time: i64,
    pub depth_latency: i64,
    pub trade_latency: Option<i64>,
}

pub trait DataProvider {
    fn get_orderbook(&self, symbol: &str) -> Result<Depth, Box<dyn std::error::Error>>;

    fn get_trades(&self, symbol: &str) -> Result<Vec<Trade>, Box<dyn std::error::Error>>;
}
