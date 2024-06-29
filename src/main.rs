#![windows_subsystem = "windows"]

mod data_providers;
use data_providers::binance::market_data;
use data_providers::{binance, bybit};
mod charts;
use charts::custom_line::{self, CustomLine};
use charts::heatmap::{self, Heatmap};
use charts::footprint::{self, Footprint};
use iced::event;

use std::vec;
use chrono::{NaiveDateTime, DateTime, Utc};
use iced::{
    alignment, font, widget::{
        button, center, checkbox, mouse_area, opaque, pick_list, stack, text_input, tooltip, Column, Container, Row, Slider, Space, Text
    }, Alignment, Color, Task, Element, Font, Length, Renderer, Settings, Size, Subscription, Theme
};

use iced::widget::pane_grid::{self, PaneGrid, Configuration};
use iced::widget::{
    container, row, scrollable, text, responsive
};
use futures::TryFutureExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    const ALL: [Exchange; 2] = [Exchange::BinanceFutures, Exchange::BybitLinear];
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ticker {
    BTCUSDT,
    ETHUSDT,
    SOLUSDT,
    LTCUSDT,
}
impl Ticker {
    const ALL: [Ticker; 4] = [Ticker::BTCUSDT, Ticker::ETHUSDT, Ticker::SOLUSDT, Ticker::LTCUSDT];
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timeframe {
    M1,
    M3,
    M5,
    M15,
    M30,
}
impl Timeframe {
    const ALL: [Timeframe; 5] = [Timeframe::M1, Timeframe::M3, Timeframe::M5, Timeframe::M15, Timeframe::M30];
}

// binance testnet api keys
const API_KEY: &str = "";
const SECRET_KEY: &str = "";

const ICON_BYTES: &[u8] = include_bytes!("fonts/icons.ttf");
const ICON: Font = Font::with_name("icons");

enum Icon {
    Locked,
    Unlocked,
    ResizeFull,
    ResizeSmall,
    Close,
    Layout,
    Cog,
}

impl From<Icon> for char {
    fn from(icon: Icon) -> Self {
        match icon {
            Icon::Unlocked => '\u{E800}',
            Icon::Locked => '\u{E801}',
            Icon::ResizeFull => '\u{E802}',
            Icon::ResizeSmall => '\u{E803}',
            Icon::Close => '\u{E804}',
            Icon::Layout => '\u{E805}',
            Icon::Cog => '\u{E806}',
        }
    }
}

#[derive(Debug)]
enum BinanceWsState {
    Connected(binance::market_data::Connection),
    Disconnected,
}
impl Default for BinanceWsState {
    fn default() -> Self {
        Self::Disconnected
    }
}

#[derive(Debug)]
enum BybitWsState {
    Connected(bybit::market_data::Connection),
    Disconnected,
}
impl Default for BybitWsState {
    fn default() -> Self {
        Self::Disconnected
    }
}

enum UserWsState {
    Connected(binance::user_data::Connection),
    Disconnected,
}
impl Default for UserWsState {
    fn default() -> Self {
        Self::Disconnected
    }
}

#[derive(Debug, Clone, Copy)]
#[derive(Eq, Hash, PartialEq)]
pub enum PaneId {
    HeatmapChart,
    FootprintChart,
    CandlestickChart,
    CustomChart,
    TimeAndSales,
    TradePanel,
}

#[derive(Debug, Clone)]
struct PaneSpec {
    id: PaneId,
    show_modal: bool,
    stream: (Option<Ticker>, Option<Timeframe>, Option<f32>),
}

impl PaneSpec {
    fn new(id: PaneId) -> Self {
        Self { 
            id,
            show_modal: false,
            stream: (None, None, None),
        }
    }
}

fn main() -> iced::Result {
    iced::application(
        "Iced Trade",
        State::update,
        State::view,
    )
    .subscription(State::subscription)
    .theme(|_| Theme::KanagawaDragon)
    .antialiasing(true)
    .window_size(iced::Size::new(1600.0, 900.0))
    .centered()   
    .font(ICON_BYTES)
    .run()
}
impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum MarketEvents {
    Binance(binance::market_data::Event),
    Bybit(bybit::market_data::Event),
}

#[derive(Debug, Clone)]
pub enum Message {
    Debug(String),

    CustomLine(custom_line::Message),
    Candlestick(custom_line::Message),
    Heatmap(heatmap::Message),
    Footprint(footprint::Message),

    // Market&User data stream
    UserKeySucceed(String),
    UserKeyError,
    TickerSelected(Ticker),
    TimeframeSelected(Timeframe, pane_grid::Pane),
    ExchangeSelected(Exchange),
    MarketWsEvent(MarketEvents),
    WsToggle,
    FetchEvent(Result<Vec<binance::market_data::Kline>, std::string::String>, PaneId, Timeframe),
    
    // Pane grid
    Split(pane_grid::Axis, pane_grid::Pane, PaneId),
    Clicked(pane_grid::Pane),
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    Maximize(pane_grid::Pane),
    Restore,
    Close(pane_grid::Pane),
    ToggleLayoutLock,

    // Modal
    OpenModal(pane_grid::Pane),
    CloseModal,

    // Slider
    SliderChanged(PaneId, f32),
    SyncWithHeatmap(bool),

    CutTheKlineStream,

    ShowLayoutModal,
    HideLayoutModal,

    TicksizeSelected(u16),
    SetMinTickSize(f32),
    
    ErrorOccurred(String),
}

struct State {
    show_layout_modal: bool,

    candlestick_chart: Option<CustomLine>,
    time_and_sales: Option<TimeAndSales>,
    custom_line: Option<CustomLine>,
    heatmap_chart: Option<Heatmap>,
    footprint_chart: Option<Footprint>,

    // data streams
    listen_key: Option<String>,
    selected_ticker: Option<Ticker>,
    selected_exchange: Option<Exchange>,

    binance_ws_state: BinanceWsState,
    bybit_ws_state: BybitWsState,

    user_ws_state: UserWsState,
    ws_running: bool,

    // pane grid
    panes: pane_grid::State<PaneSpec>,
    focus: Option<pane_grid::Pane>,
    first_pane: pane_grid::Pane,
    pane_lock: bool,

    size_filter_timesales: f32,
    size_filter_heatmap: f32,
    sync_heatmap: bool,

    kline_stream: bool,

    tick_multiply: u16,
    min_tick_size: Option<f32>,
}

impl State {
    fn new() -> Self {
        let pane_config: Configuration<PaneSpec> = Configuration::Split {
            axis: pane_grid::Axis::Vertical,
            ratio: 0.8,
            a: Box::new(Configuration::Split {
                axis: pane_grid::Axis::Horizontal,
                ratio: 0.4,
                a: Box::new(Configuration::Split {
                    axis: pane_grid::Axis::Vertical,
                    ratio: 0.5,
                    a: Box::new(Configuration::Pane(
                        PaneSpec { 
                            id: PaneId::CandlestickChart, 
                            show_modal: false, 
                            stream: (Some(Ticker::BTCUSDT), Some(Timeframe::M1), None)
                        })
                    ),
                    b: Box::new(Configuration::Pane(
                        PaneSpec { 
                            id: PaneId::CustomChart, 
                            show_modal: false, 
                            stream: (Some(Ticker::BTCUSDT), Some(Timeframe::M15), None)
                        })
                    ),
                }),
                b: Box::new(Configuration::Split {
                    axis: pane_grid::Axis::Vertical,
                    ratio: 0.5,
                    a: Box::new(Configuration::Pane(
                        PaneSpec { 
                            id: PaneId::FootprintChart, 
                            show_modal: false, 
                            stream: (Some(Ticker::BTCUSDT), Some(Timeframe::M3), Some(1.0))
                        })                      
                    ),
                    b: Box::new(Configuration::Pane(
                        PaneSpec { 
                            id: PaneId::HeatmapChart, 
                            show_modal: false, 
                            stream: (Some(Ticker::BTCUSDT), None, None)
                        })
                    ),
                }),
            }),
            b: Box::new(Configuration::Pane(
                PaneSpec { 
                    id: PaneId::TimeAndSales, 
                    show_modal: false, 
                    stream: (Some(Ticker::BTCUSDT), None, None) 
                })
            ),
        };
        let panes: pane_grid::State<PaneSpec> = pane_grid::State::with_configuration(pane_config);
        let first_pane: pane_grid::Pane = *panes.panes.iter().next().unwrap().0;
        
        Self { 
            show_layout_modal: false,

            size_filter_timesales: 0.0,
            size_filter_heatmap: 0.0,
            sync_heatmap: false,
            kline_stream: true,

            candlestick_chart: None,
            time_and_sales: None,
            custom_line: None,
            heatmap_chart: None,
            footprint_chart: None,

            listen_key: None,
            selected_ticker: None,
            selected_exchange: Some(Exchange::BinanceFutures),
            binance_ws_state: BinanceWsState::Disconnected,
            bybit_ws_state: BybitWsState::Disconnected,
            user_ws_state: UserWsState::Disconnected,
            ws_running: false,
            panes,
            focus: None,
            first_pane,
            pane_lock: false,
            tick_multiply: 1,
            min_tick_size: None, 
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CustomLine(message) => {
                if let Some(custom_line) = &mut self.custom_line {
                    custom_line.update(&message);
                }
                Task::none()
            },
            Message::Candlestick(message) => {
                if let Some(candlesticks) = &mut self.candlestick_chart {
                    candlesticks.update(&message);
                }
                Task::none()
            },
            Message::Heatmap(message) => {
                if let Some(heatmap) = &mut self.heatmap_chart {
                    heatmap.update(&message);
                }
                Task::none()
            },
            Message::Footprint(message) => {
                if let Some(footprint) = &mut self.footprint_chart {
                    footprint.update(&message);
                }
                Task::none()
            },

            Message::SetMinTickSize(min_tick_size) => {
                self.min_tick_size = Some(min_tick_size);

                if let Some(heatmap_chart) = &mut self.heatmap_chart {
                    let copied_trades = heatmap_chart.get_raw_trades();

                    if let Some(footprint_chart) = &mut self.footprint_chart {
                        footprint_chart.change_tick_size(copied_trades, self.tick_multiply as f32 * self.min_tick_size.unwrap_or(1.0));
                    }
                }

                Task::none()
            },
            Message::TickerSelected(ticker) => {
                self.selected_ticker = Some(ticker);

                let panes_state = self.panes.iter_mut();
                for (pane_id, pane_state) in panes_state {
                    pane_state.stream.0 = Some(ticker);
                }

                Task::none()
            },
            Message::TicksizeSelected(tick_multiply) => {
                if let Some(heatmap_chart) = &mut self.heatmap_chart {
                    let copied_trades = heatmap_chart.get_raw_trades();

                    if let Some(footprint_chart) = &mut self.footprint_chart {
                        footprint_chart.change_tick_size(copied_trades, tick_multiply as f32 * self.min_tick_size.unwrap_or(1.0));

                        self.tick_multiply = tick_multiply;
                    }
                }

                Task::none()
            },
            Message::TimeframeSelected(timeframe, pane) => {
                if !self.ws_running {
                    return Task::none();
                }

                let Some(selected_ticker) = &self.selected_ticker else {
                    eprintln!("No ticker selected");
                    return Task::none();
                };

                self.kline_stream = false;
                
                let mut tasks = vec![];

                if let Some(pane) = self.panes.panes.get_mut(&pane) {
                    let pane_id = pane.id;

                    pane.stream.1 = Some(timeframe);

                    match self.selected_exchange {
                        Some(Exchange::BinanceFutures) => {
                            let fetch_klines = Task::perform(
                                market_data::fetch_klines(*selected_ticker, timeframe)
                                    .map_err(|err| format!("{err}")), 
                                move |klines| {
                                    Message::FetchEvent(klines, pane_id, timeframe)
                                }
                            );
                            
                            tasks.push(fetch_klines);
                        },
                        Some(Exchange::BybitLinear) => {
                            let fetch_klines: Task<Message> = Task::perform(
                                bybit::market_data::fetch_klines(self.selected_ticker.unwrap_or(Ticker::BTCUSDT), timeframe)
                                    .map_err(|err| format!("{err}")), 
                                move |klines: Result<Vec<bybit::market_data::Kline>, String>| {

                                    match klines {
                                        Ok(klines) => {
                                            let binance_klines: Vec<market_data::Kline> = klines.iter().map(|kline| {
                                                market_data::Kline {
                                                    time: kline.time,
                                                    open: kline.open,
                                                    high: kline.high,
                                                    low: kline.low,
                                                    close: kline.close,
                                                    volume: kline.volume,
                                                    taker_buy_base_asset_volume: -1.0,
                                                }
                                            }).collect();

                                            Message::FetchEvent(Ok(binance_klines), pane_id, timeframe)
                                        },
                                        Err(err) => {
                                            Message::Debug(err)
                                        }
                                    }
                                }
                            );
                            
                            tasks.push(fetch_klines);
                        },
                        None => {
                            eprintln!("No exchange selected");
                        }
                    }                               
                };
        
                // sleep to drop existent stream and create new one
                let remove_active_stream = Task::perform(
                    async {
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    },
                    move |()| Message::CutTheKlineStream
                );
                tasks.push(remove_active_stream);

                Task::batch(tasks)
            },
            Message::ExchangeSelected(exchange) => {
                self.selected_exchange = Some(exchange);
                Task::none()
            },
            Message::WsToggle => {
                self.ws_running = !self.ws_running;

                if self.ws_running {  
                    let mut tasks: Vec<Task<Message>> = vec![];
        
                    for (_, pane_state) in self.panes.iter() {
                        if pane_state.id == PaneId::HeatmapChart {
                            self.heatmap_chart = Some(Heatmap::new());
                        }
                        if pane_state.id == PaneId::TimeAndSales {
                            self.time_and_sales = Some(TimeAndSales::new());
                        }
                        if pane_state.id == PaneId::FootprintChart {
                            let fetch_ticksize: Task<Message> = Task::perform(
                                bybit::market_data::fetch_ticksize(self.selected_ticker.unwrap_or(Ticker::BTCUSDT)),
                                move |result| match result {
                                    Ok(ticksize) => Message::SetMinTickSize(ticksize),
                                    Err(err) => {
                                        Message::ErrorOccurred(err.to_string())
                                    }
                                }
                            );
                            tasks.push(fetch_ticksize);
                        }

                        if let Some(selected_timeframe) = pane_state.stream.1 {

                            let pane_id: PaneId = pane_state.id;

                            match self.selected_exchange {
                                Some(Exchange::BinanceFutures) => {
                                    let fetch_klines: Task<Message> = Task::perform(
                                        market_data::fetch_klines(self.selected_ticker.unwrap_or(Ticker::BTCUSDT), selected_timeframe)
                                            .map_err(|err| format!("{err}")), 
                                        move |klines: Result<Vec<market_data::Kline>, String>| {
                                            Message::FetchEvent(klines, pane_id, selected_timeframe)
                                        }
                                    );
                                    tasks.push(fetch_klines);
                                },
                                Some(Exchange::BybitLinear) => {
                                    let fetch_klines: Task<Message> = Task::perform(
                                        bybit::market_data::fetch_klines(self.selected_ticker.unwrap_or(Ticker::BTCUSDT), selected_timeframe)
                                            .map_err(|err| format!("{err}")), 
                                        move |klines: Result<Vec<bybit::market_data::Kline>, String>| {

                                            match klines {
                                                Ok(klines) => {
                                                    let binance_klines: Vec<market_data::Kline> = klines.iter().map(|kline| {
                                                        market_data::Kline {
                                                            time: kline.time,
                                                            open: kline.open,
                                                            high: kline.high,
                                                            low: kline.low,
                                                            close: kline.close,
                                                            volume: kline.volume,
                                                            taker_buy_base_asset_volume: -1.0,
                                                        }
                                                    }).collect();

                                                    Message::FetchEvent(Ok(binance_klines), pane_id, selected_timeframe)
                                                },
                                                Err(err) => {
                                                    Message::Debug(err)
                                                }
                                            }
                                        }
                                    );
                                    tasks.push(fetch_klines);
                                },
                                None => {
                                    eprintln!("No exchange selected");
                                }
                            }
                        }
                    };
                    
                    Task::batch(tasks)

                } else {
                    self.binance_ws_state = BinanceWsState::Disconnected;
                    self.bybit_ws_state = BybitWsState::Disconnected;

                    self.heatmap_chart = None;
                    self.candlestick_chart = None;
                    self.time_and_sales = None;
                    self.custom_line = None;
                    self.footprint_chart = None;

                    Task::none()
                }
            },       
            Message::FetchEvent(klines, target_pane, timeframe) => {
                match klines {
                    Ok(klines) => {
                        match target_pane {
                            PaneId::CustomChart => {
                                self.custom_line = Some(CustomLine::new(klines, timeframe));
                            },
                            PaneId::CandlestickChart => {
                                self.candlestick_chart = Some(CustomLine::new(klines, timeframe));
                            },
                            PaneId::FootprintChart => {
                                if let Some(heatmap_chart) = &mut self.heatmap_chart {
                                    let copied_trades: Vec<Trade> = heatmap_chart.get_raw_trades();

                                    let mut klines_raw: Vec<(i64, f32, f32, f32, f32, f32, f32)> = vec![];
                                    for kline in &klines {
                                        let buy_volume = kline.taker_buy_base_asset_volume;
                                        let sell_volume = kline.volume - buy_volume;

                                        klines_raw.push((kline.time as i64, kline.open, kline.high, kline.low, kline.close, buy_volume, sell_volume));
                                    }

                                    let timeframe_u16: u16 = match timeframe {
                                        Timeframe::M1 => 1,
                                        Timeframe::M3 => 3,
                                        Timeframe::M5 => 5,
                                        Timeframe::M15 => 15,
                                        Timeframe::M30 => 30,
                                    };

                                    let tick_size = self.tick_multiply as f32 * self.min_tick_size.unwrap_or_default();

                                    self.footprint_chart = Some(Footprint::new(timeframe_u16, tick_size, klines_raw, copied_trades));
                                }
                            },
                            _ => {}
                        }
                    },
                    Err(err) => {
                        eprintln!("Error fetching klines: {err}");
                        self.candlestick_chart = Some(CustomLine::new(vec![], Timeframe::M1)); 
                    },
                }
                Task::none()
            },
            Message::MarketWsEvent(event) => {
                match event {
                    MarketEvents::Binance(event) => match event {
                        binance::market_data::Event::Connected(connection) => {
                            self.binance_ws_state = BinanceWsState::Connected(connection);
                        }
                        binance::market_data::Event::Disconnected => {
                            self.binance_ws_state = BinanceWsState::Disconnected;
                        }
                        binance::market_data::Event::DepthReceived(depth_update, depth, trades_buffer) => {
                            if let Some(time_and_sales) = &mut self.time_and_sales {
                                time_and_sales.update(&trades_buffer);
                            } 

                            let trades_buffer_clone = trades_buffer.clone();

                            if let Some(chart) = &mut self.heatmap_chart {
                                chart.insert_datapoint(trades_buffer, depth_update, depth);
                            } 
                            if let Some(chart) = &mut self.footprint_chart {
                                chart.insert_datapoint(trades_buffer_clone, depth_update);
                            }
                        }
                        binance::market_data::Event::KlineReceived(kline, timeframe) => {
                            for (_, pane_state) in self.panes.iter() {
                                if let Some(selected_timeframe) = pane_state.stream.1 {
                                    if selected_timeframe == timeframe {
                                        match pane_state.id {
                                            PaneId::CandlestickChart => {
                                                if let Some(candlestick_chart) = &mut self.candlestick_chart {
                                                    candlestick_chart.insert_datapoint(&kline);
                                                }
                                            },
                                            PaneId::CustomChart => {
                                                if let Some(custom_line) = &mut self.custom_line {
                                                    custom_line.insert_datapoint(&kline);
                                                }
                                            },
                                            PaneId::FootprintChart => {
                                                if let Some(footprint_chart) = &mut self.footprint_chart {
                                                    footprint_chart.update_latest_kline(&kline);
                                                }
                                            },
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    },

                    MarketEvents::Bybit(event) => match event {
                        bybit::market_data::Event::Connected(connection) => {
                            self.bybit_ws_state = BybitWsState::Connected(connection);

                            println!("Bybit connected");
                        }
                        bybit::market_data::Event::Disconnected => {
                            self.bybit_ws_state = BybitWsState::Disconnected;

                            println!("Bybit disconnected");
                        }
                        bybit::market_data::Event::DepthReceived(depth_update, depth, trades_buffer) => {

                            // convert bybit trade to binance trade
                            let mut binance_trades: Vec<binance::market_data::Trade> = vec![];
                            
                            for trade in trades_buffer.iter() {
                                let binance_trade = binance::market_data::Trade {
                                    price: trade.price,
                                    qty: trade.qty,
                                    time: trade.time,
                                    is_sell: trade.is_sell,
                                };
                                binance_trades.push(binance_trade);
                            }

                            let local_depth = binance::market_data::LocalDepthCache {
                                time: depth.time,
                                bids: depth.bids.iter().map(|order| binance::market_data::Order { price: order.price, qty: order.qty }).collect(),
                                asks: depth.asks.iter().map(|order| binance::market_data::Order { price: order.price, qty: order.qty }).collect(),
                            };

                            let trades_clone = binance_trades.clone();

                            if let Some(time_and_sales) = &mut self.time_and_sales {
                                time_and_sales.update(&binance_trades);
                            } 

                            if let Some(chart) = &mut self.heatmap_chart {
                                chart.insert_datapoint(binance_trades, depth_update, local_depth);
                            } 
                            if let Some(chart) = &mut self.footprint_chart {
                                chart.insert_datapoint(trades_clone, depth_update);
                            }
                        }
                        bybit::market_data::Event::KlineReceived(kline, timeframe) => {
                            for (_, pane_state) in self.panes.iter() {
                                if let Some(selected_timeframe) = pane_state.stream.1 {
                                    if selected_timeframe == timeframe {
                                        let binance_kline = binance::market_data::Kline {
                                            time: kline.time,
                                            open: kline.open,
                                            high: kline.high,
                                            low: kline.low,
                                            close: kline.close,
                                            volume: kline.volume,
                                            taker_buy_base_asset_volume: -1.0,
                                        };

                                        match pane_state.id {
                                            PaneId::CandlestickChart => {
                                                if let Some(candlestick_chart) = &mut self.candlestick_chart {
                                                    candlestick_chart.insert_datapoint(&binance_kline);
                                                }
                                            },
                                            PaneId::CustomChart => {
                                                if let Some(custom_line) = &mut self.custom_line {
                                                    custom_line.insert_datapoint(&binance_kline);
                                                }
                                            },
                                            PaneId::FootprintChart => {
                                                if let Some(footprint_chart) = &mut self.footprint_chart {
                                                    footprint_chart.update_latest_kline(&binance_kline);
                                                }
                                            },
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                };
                Task::none()
            },
            Message::UserKeySucceed(listen_key) => {
                self.listen_key = Some(listen_key);
                Task::none()
            },
            Message::UserKeyError => {
                eprintln!("Check API keys");
                Task::none()
            },

            // Pane grid
            Message::Split(axis, pane, pane_id) => {
                let focus_pane = if let Some((pane, _)) = self.panes.split(axis, pane, PaneSpec::new(pane_id)) {
                    Some(pane)
                } else if let Some((&first_pane, _)) = self.panes.panes.iter().next() {
                    self.panes.split(axis, first_pane, PaneSpec::new(pane_id)).map(|(pane, _)| pane)
                } else {
                    None
                };

                if Some(focus_pane).is_some() {
                    self.focus = focus_pane;
                } 

                Task::none()
            },
            Message::Clicked(pane) => {
                self.focus = Some(pane);
                Task::none()
            },
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Task::none()
            },
            Message::Dragged(pane_grid::DragEvent::Dropped {
                pane,
                target,
            }) => {
                self.panes.drop(pane, target);
                Task::none()
            },
            Message::Dragged(_) => {
                Task::none()
            },
            Message::Maximize(pane) => {
                self.panes.maximize(pane);
                Task::none()
            },
            Message::Restore => {
                self.panes.restore();
                Task::none()
            },
            Message::Close(pane) => {                
                if let Some((_, sibling)) = self.panes.close(pane) {
                    self.focus = Some(sibling);
                }
                Task::none()
            },
            Message::ToggleLayoutLock => {
                self.focus = None;
                self.pane_lock = !self.pane_lock;
                Task::none()
            },

            Message::Debug(_msg) => {
                let layout = self.panes.layout();
                dbg!(layout);
                let state_config = &self.panes.panes;
                dbg!(state_config);
                Task::none()
            },

            Message::OpenModal(pane) => {
                if let Some(pane) = self.panes.get_mut(pane) {
                    pane.show_modal = true;
                };
                Task::none()
            },
            Message::CloseModal => {
                for pane in self.panes.panes.values_mut() {
                    pane.show_modal = false;
                }
                Task::none()
            },

            Message::SliderChanged(pane_id, value) => {
                if pane_id == PaneId::TimeAndSales {
                    self.size_filter_timesales = value;
                    if self.sync_heatmap {
                        self.size_filter_heatmap = value;
                    }
                } else if pane_id == PaneId::HeatmapChart {
                    self.size_filter_heatmap = value;
                    self.sync_heatmap = false;
                }

                if let Some(heatmap_chart) = &mut self.heatmap_chart {
                    heatmap_chart.set_size_filter(self.size_filter_heatmap);
                }
                if let Some(time_and_sales) = &mut self.time_and_sales {
                    time_and_sales.set_size_filter(self.size_filter_timesales);
                };

                Task::none()
            },
            Message::SyncWithHeatmap(sync) => {
                self.sync_heatmap = sync;
            
                if sync {
                    self.size_filter_heatmap = self.size_filter_timesales;
                    if let Some(heatmap_chart) = &mut self.heatmap_chart {
                        heatmap_chart.set_size_filter(self.size_filter_heatmap);
                    }
                }
            
                Task::none()
            },
            Message::CutTheKlineStream => {
                self.kline_stream = true;
                Task::none()
            },

            Message::ShowLayoutModal => {
                self.show_layout_modal = true;
                iced::widget::focus_next()
            },
            Message::HideLayoutModal => {
                self.show_layout_modal = false;
                Task::none()
            },

            Message::ErrorOccurred(err) => {
                eprintln!("{err}");
                Task::none()
            },
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let focus = self.focus;
        let total_panes = self.panes.len();

        let pane_grid = PaneGrid::new(&self.panes, |id, pane, is_maximized| {
            let is_focused = focus == Some(id);
    
            let content: pane_grid::Content<'_, Message, _, Renderer> = pane_grid::Content::new(responsive(move |size| {
                view_content(
                    pane.id, 
                    pane.show_modal,
                    &self.size_filter_heatmap,
                    &self.size_filter_timesales,
                    self.sync_heatmap,
                    total_panes, 
                    size, 
                    &self.footprint_chart,
                    &self.heatmap_chart,
                    &self.time_and_sales,
                    &self.candlestick_chart, 
                    &self.custom_line,
                )
            }));
    
            if self.pane_lock {
                return content.style(style::pane_active);
            }
    
            let mut content = content.style(if is_focused {
                style::pane_focused
            } else {
                style::pane_active
            });
        
            if is_focused {
                let title = match pane.id {
                    PaneId::HeatmapChart => "Heatmap",
                    PaneId::FootprintChart => "Footprint",
                    PaneId::CandlestickChart => "Candlesticks",
                    PaneId::CustomChart => "Candlesticks",
                    PaneId::TimeAndSales => "Time&Sales",
                    PaneId::TradePanel => "Trade Panel",
                };

                let title_bar = pane_grid::TitleBar::new(title)
                    .always_show_controls()
                    .controls(view_controls(
                        id,
                        pane.id,
                        total_panes,
                        is_maximized,
                        pane.stream.1.as_ref(),
                        self.tick_multiply,
                    ))
                    .padding(4)
                    .style(style::title_bar_focused);
                content = content.title_bar(title_bar);
            }
            content
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(10)
        .on_click(Message::Clicked)
        .on_drag(Message::Dragged)
        .on_resize(10, Message::Resized);

        let layout_lock_button = button(
            container(
                if self.pane_lock { 
                    text(char::from(Icon::Locked).to_string()).font(ICON) 
                } else { 
                    text(char::from(Icon::Unlocked).to_string()).font(ICON) 
                })
                .width(25)
                .center_x(iced::Pixels(20.0))
            )
            .on_press(Message::ToggleLayoutLock);

        let add_pane_button = button(
            container(
                text(char::from(Icon::Layout).to_string()).font(ICON))
                .width(25)
                .center_x(iced::Pixels(20.0))
            )
            .on_press(Message::ShowLayoutModal);

        let layout_controls = Row::new()
            .spacing(10)
            .align_items(Alignment::Center)
            .push(
                tooltip(add_pane_button, "Manage Panes", tooltip::Position::Bottom).style(style::tooltip))
            .push(
                tooltip(layout_lock_button, "Layout Lock", tooltip::Position::Bottom).style(style::tooltip)
            );

        let ws_button = if self.selected_ticker.is_some() {
            button(if self.ws_running { "Disconnect" } else { "Connect" })
                .on_press(Message::WsToggle)
        } else {
            button(if self.ws_running { "Disconnect" } else { "Connect" })
        };
        let mut ws_controls = Row::new()
            .spacing(10)
            .align_items(Alignment::Center)
            .push(ws_button);

        if self.ws_running {
            ws_controls = ws_controls.push(
                Text::new(self.selected_ticker.unwrap_or_else(|| { dbg!("No ticker found"); Ticker::BTCUSDT } ).to_string()).size(20));
        } else {
            let symbol_pick_list = pick_list(
                &Ticker::ALL[..],
                self.selected_ticker,
                Message::TickerSelected,
            ).placeholder("Choose a ticker...");
            
            let exchange_selector = pick_list(
                &Exchange::ALL[..],
                self.selected_exchange,
                Message::ExchangeSelected,
            ).placeholder("Choose an exchange...");

            ws_controls = ws_controls
                .push(exchange_selector)
                .push(symbol_pick_list);
        }

        let content = Column::new()
            .padding(10)
            .spacing(10)
            .align_items(Alignment::Start)
            .width(Length::Fill)
            .height(Length::Fill)
            .push(
                Row::new()
                    .spacing(10)
                    .push(ws_controls)
                    .push(Space::with_width(Length::Fill))
                    .push(button("Debug").on_press(Message::Debug("Debug".to_string())))                
                    .push(layout_controls)
            )
            .push(pane_grid);

        if self.show_layout_modal {
            let mut buttons = Column::new().spacing(2).align_items(Alignment::Start);

            let pane_info = vec![
                (PaneId::HeatmapChart, "Heatmap Chart"),
                (PaneId::FootprintChart, "Footprint Chart"),
                (PaneId::CandlestickChart, "Candlestick Chart"),
                (PaneId::CustomChart, "Custom Chart"),
                (PaneId::TimeAndSales, "Time & Sales"),
            ];

            for (pane_id, label) in pane_info {
                let button = button(label).width(iced::Pixels(200.0));

                if self.panes.iter().any(|(_p, ps)| ps.id == pane_id) {
                    buttons = buttons.push(button);
                } else {
                    let message = Message::Split(pane_grid::Axis::Vertical, self.first_pane, pane_id);
                    buttons = buttons.push(button.on_press(message));
                }
            }

            let signup = container(
                Column::new()
                    .spacing(10)
                    .align_items(Alignment::Center)
                    .push(
                        Text::new("Add a new pane")
                            .size(20)
                    )
                    .push(buttons)
                    .push(
                        Column::new()
                            .align_items(Alignment::Center)
                            .push(
                                button("Close")
                                    .on_press(Message::HideLayoutModal)
                            )
                    )
            )
            .width(Length::Shrink)
            .padding(20)
            .style(style::title_bar_active);
            modal(content, signup, Message::HideLayoutModal)
        } else {
            content.into()
        }  
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = Vec::new();

        if self.ws_running {
            if let Some(ticker) = &self.selected_ticker {
                match self.selected_exchange {
                    Some(Exchange::BinanceFutures) => {
                        let binance_market_stream: Subscription<Message> = binance::market_data::connect_market_stream(*ticker)
                            .map(|arg0: binance::market_data::Event| Message::MarketWsEvent(MarketEvents::Binance(arg0)));

                        subscriptions.push(binance_market_stream);

                        let mut streams: Vec<(Ticker, Timeframe)> = vec![];

                        for (_, pane_state) in self.panes.iter() {
                            if let (Some(ticker), Some(timeframe)) = (pane_state.stream.0, pane_state.stream.1) {
                                streams.push((ticker, timeframe));
                            }
                        }

                        if !streams.is_empty() && self.kline_stream {
                            let binance_kline_streams: Subscription<Message> = binance::market_data::connect_kline_stream(streams)
                                .map(|arg0: binance::market_data::Event| Message::MarketWsEvent(MarketEvents::Binance(arg0)));

                            subscriptions.push(binance_kline_streams);
                        }
                    },

                    Some(Exchange::BybitLinear) => {
                        let bybit_market_stream: Subscription<Message> = bybit::market_data::connect_market_stream(*ticker)
                            .map(|arg0: bybit::market_data::Event| Message::MarketWsEvent(MarketEvents::Bybit(arg0)));

                        subscriptions.push(bybit_market_stream);

                        let mut streams: Vec<(Ticker, Timeframe)> = vec![];

                        for (_, pane_state) in self.panes.iter() {
                            if let (Some(ticker), Some(timeframe)) = (pane_state.stream.0, pane_state.stream.1) {
                                streams.push((ticker, timeframe));
                            }
                        }

                        if !streams.is_empty() && self.kline_stream {
                            let bybit_kline_streams: Subscription<Message> = bybit::market_data::connect_kline_stream(streams)
                                .map(|arg0: bybit::market_data::Event| Message::MarketWsEvent(MarketEvents::Bybit(arg0)));

                            subscriptions.push(bybit_kline_streams);
                        }
                    },

                    None => {
                        println!("No exchange selected");
                    },
                }
            }
        }
        
        Subscription::batch(subscriptions)
    }    
}

fn modal<'a, Message>(
    base: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
    on_blur: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    stack![
        base.into(),
        mouse_area(center(opaque(content)).style(|_theme| {
            container::Style {
                background: Some(
                    Color {
                        a: 0.8,
                        ..Color::BLACK
                    }
                    .into(),
                ),
                ..container::Style::default()
            }
        }))
        .on_press(on_blur)
    ]
    .into()
}

fn view_content<'a, 'b: 'a>(
    pane_id: PaneId,
    show_modal: bool,
    size_filter_heatmap: &'a f32,
    size_filter_timesales: &'a f32,
    sync_heatmap: bool,
    _total_panes: usize,
    _size: Size,
    footprint_chart: &'a Option<Footprint>,
    heatmap_chart: &'a Option<Heatmap>,
    time_and_sales: &'a Option<TimeAndSales>,
    candlestick_chart: &'a Option<CustomLine>,
    custom_line: &'a Option<CustomLine>,
) -> Element<'a, Message> {
    let content: Element<Message, Theme, Renderer> = match pane_id {
        PaneId::HeatmapChart => {
            let underlay; 
            if let Some(heatmap_chart) = heatmap_chart {
                underlay =
                    heatmap_chart
                        .view()
                        .map(Message::Heatmap);
            } else {
                underlay = Text::new("No data")
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
            }

            if show_modal {
                let signup: Container<Message, Theme, _> = container(
                    Column::new()
                        .spacing(10)
                        .align_items(Alignment::Center)
                        .push(
                            Text::new("Heatmap > Settings")
                                .size(16)
                        )
                        .push(
                            Column::new()
                                .align_items(Alignment::Center)
                                .push(Text::new("Size Filtering"))
                                .push(
                                    Slider::new(0.0..=50000.0, *size_filter_heatmap, move |value| Message::SliderChanged(PaneId::HeatmapChart, value))
                                        .step(500.0)
                                )
                                .push(
                                    Text::new(format!("${size_filter_heatmap}")).size(16)
                                )
                        )
                        .push( 
                            Row::new()
                                .spacing(10)
                                .push(
                                    button("Close")
                                    .on_press(Message::CloseModal)
                                )
                        )
                )
                .width(Length::Shrink)
                .padding(20)
                .max_width(500)
                .style(style::title_bar_active);

                return modal(underlay, signup, Message::CloseModal);
            } else {
                underlay
            }
        }, 

        PaneId::FootprintChart => { 
            let underlay; 
            if let Some(footprint_chart) = footprint_chart {
                underlay =
                    footprint_chart
                        .view()
                        .map(Message::Footprint);
            } else {
                underlay = Text::new("No data")
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
            }
            underlay
        },
        
        PaneId::CandlestickChart => { 
            let underlay; 
            if let Some(candlestick_chart) = candlestick_chart {
                underlay =
                    candlestick_chart
                        .view()
                        .map(Message::Candlestick);
            } else {
                underlay = Text::new("No data")
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
            }
            underlay
        },

        PaneId::CustomChart => { 
            let underlay; 
            if let Some(custom_line) = custom_line {
                underlay =
                    custom_line
                        .view()
                        .map(Message::CustomLine);
            } else {
                underlay = Text::new("No data")
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
            }
            underlay
        },
        
        PaneId::TimeAndSales => { 
            let underlay = time_and_sales.as_ref().map_or_else(
                || Text::new("No data")
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                TimeAndSales::view
            );

            if show_modal {
                let signup = container(
                    Column::new()
                        .spacing(10)
                        .align_items(Alignment::Center)
                        .push(
                            Text::new("Time&Sales > Settings")
                                .size(16)
                        )
                        .push(
                            Column::new()
                                .align_items(Alignment::Center)
                                .push(Text::new("Size Filtering"))
                                .push(
                                    Slider::new(0.0..=50000.0, *size_filter_timesales, move |value| Message::SliderChanged(PaneId::TimeAndSales, value))
                                        .step(500.0)
                                )
                                .push(
                                    Text::new(format!("${size_filter_timesales}")).size(16)
                                )
                                .push(
                                    checkbox("Sync Heatmap with", sync_heatmap)
                                        .on_toggle(Message::SyncWithHeatmap)
                                )
                        )
                        .push( 
                            Row::new()
                                .spacing(10)
                                .push(
                                    button("Close")
                                    .on_press(Message::CloseModal)
                                )
                        )
                )
                .width(Length::Shrink)
                .padding(20)
                .max_width(500)
                .style(style::title_bar_active);

                return modal(underlay, signup, Message::CloseModal);
            } else {
                underlay
            }
        },  
        
        PaneId::TradePanel => {
            Text::new("No account info found").into()
        },
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_controls<'a>(
    pane: pane_grid::Pane,
    pane_id: PaneId,
    total_panes: usize,
    is_maximized: bool,
    selected_timeframe: Option<&'a Timeframe>,
    selected_ticksize: u16,
) -> Element<'a, Message> {
    let mut row = row![].spacing(5);

    let (icon, message) = if is_maximized {
        (Icon::ResizeSmall, Message::Restore)
    } else {
        (Icon::ResizeFull, Message::Maximize(pane))
    };

    if pane_id == PaneId::CandlestickChart || pane_id == PaneId::CustomChart || pane_id == PaneId::FootprintChart {
        let timeframe_picker = pick_list(
            &Timeframe::ALL[..],
            selected_timeframe,
            move |timeframe| Message::TimeframeSelected(timeframe, pane),
        ).placeholder("Choose a timeframe...").text_size(11).width(iced::Pixels(80.0));
        row = row.push(timeframe_picker);
    }
    if pane_id == PaneId::FootprintChart {
        let ticksize_picker = pick_list(
            [1, 2, 5, 10, 25, 50, 100, 200],
            Some(selected_ticksize),
            Message::TicksizeSelected,
        ).placeholder("Ticksize multiplier...").text_size(11).width(iced::Pixels(80.0));
        row = row.push(ticksize_picker);
    }

    let mut buttons = vec![
        (container(text(char::from(Icon::Cog).to_string()).font(ICON).size(14)).width(25).center_x(iced::Pixels(25.0)), Message::OpenModal(pane)),
        (container(text(char::from(icon).to_string()).font(ICON).size(14)).width(25).center_x(iced::Pixels(25.0)), message),
    ];

    if total_panes > 1 {
        buttons.push((container(text(char::from(Icon::Close).to_string()).font(ICON).size(14)).width(25).center_x(iced::Pixels(25.0)), Message::Close(pane)));
    }

    for (content, message) in buttons {        
        row = row.push(
            button(content)
                .padding(3)
                .on_press(message),
        );
    } 

    row.into()
}

use crate::market_data::Trade;
struct ConvertedTrade {
    time: NaiveDateTime,
    price: f32,
    qty: f32,
    is_sell: bool,
}
struct TimeAndSales {
    recent_trades: Vec<ConvertedTrade>,
    size_filter: f32,
}
impl TimeAndSales {
    fn new() -> Self {
        Self {
            recent_trades: Vec::new(),
            size_filter: 0.0,
        }
    }
    fn set_size_filter(&mut self, value: f32) {
        self.size_filter = value;
    }

    fn update(&mut self, trades_buffer: &Vec<Trade>) {
        for trade in trades_buffer {
            let trade_time = NaiveDateTime::from_timestamp(trade.time / 1000, (trade.time % 1000) as u32 * 1_000_000);
            let converted_trade = ConvertedTrade {
                time: trade_time,
                price: trade.price,
                qty: trade.qty,
                is_sell: trade.is_sell,
            };
            self.recent_trades.push(converted_trade);
        }

        if self.recent_trades.len() > 2000 {
            let drain_to = self.recent_trades.len() - 2000;
            self.recent_trades.drain(0..drain_to);
        }
    }
    fn view(&self) -> Element<'_, Message> {
        let mut trades_column = Column::new()
            .height(Length::Fill)
            .padding(10);

        let filtered_trades: Vec<_> = self.recent_trades.iter().filter(|trade| (trade.qty*trade.price) >= self.size_filter).collect();

        let max_qty = filtered_trades.iter().map(|trade| trade.qty).fold(0.0, f32::max);
    
        if filtered_trades.is_empty() {
            trades_column = trades_column.push(
                Text::new("No trades")
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .size(16)
            );
        } else {
            for trade in filtered_trades.iter().rev().take(80) {
                let trade: &ConvertedTrade = trade;

                let trade_row = Row::new()
                    .push(
                        container(Text::new(format!("{}", trade.time.format("%M:%S.%3f"))).size(14))
                            .width(Length::FillPortion(8)).align_x(alignment::Horizontal::Center)
                    )
                    .push(
                        container(Text::new(format!("{}", trade.price)).size(14))
                            .width(Length::FillPortion(6))
                    )
                    .push(
                        container(Text::new(if trade.is_sell { "Sell" } else { "Buy" }).size(14))
                            .width(Length::FillPortion(4)).align_x(alignment::Horizontal::Left)
                    )
                    .push(
                        container(Text::new(format!("{}", trade.qty)).size(14))
                            .width(Length::FillPortion(4))
                    );

                let color_alpha = trade.qty / max_qty;
    
                trades_column = trades_column.push(container(trade_row)
                    .style( move |_| if trade.is_sell { style::sell_side_red(color_alpha) } else { style::buy_side_green(color_alpha) }));
    
                trades_column = trades_column.push(Container::new(Space::new(Length::Fixed(0.0), Length::Fixed(5.0))));
            }
        }
    
        trades_column.into()  
    }    
}

mod style {
    use iced::widget::container::Style;
    use iced::{theme, Border, Color, Theme};

    fn styled(pair: theme::palette::Pair) -> Style {
        Style {
            background: Some(pair.color.into()),
            text_color: pair.text.into(),
            ..Default::default()
        }
    }

    pub fn primary(theme: &Theme) -> Style {
        let palette = theme.extended_palette();

        styled(palette.primary.weak)
    }

    pub fn tooltip(theme: &Theme) -> Style {
        let palette = theme.extended_palette();

        Style {
            background: Some(palette.background.weak.color.into()),
            border: Border {
                width: 1.0,
                color: palette.primary.weak.color,
                radius: 4.0.into(),
            },
            ..Default::default()
        }
    }

    pub fn title_bar_active(theme: &Theme) -> Style {
        let palette = theme.extended_palette();

        Style {
            text_color: Some(palette.background.strong.text),
            background: Some(palette.background.strong.color.into()),
            border: Border {
                width: 1.0,
                color: palette.primary.strong.color,
                radius: 4.0.into(), 
            },
            ..Default::default()
        }
    }
    pub fn title_bar_focused(theme: &Theme) -> Style {
        let palette = theme.extended_palette();

        Style {
            text_color: Some(palette.primary.strong.text),
            background: Some(palette.primary.strong.color.into()),
            ..Default::default()
        }
    }
    pub fn pane_active(theme: &Theme) -> Style {
        let palette = theme.extended_palette();

        Style {
            background: Some(Color::BLACK.into()),
            border: Border {
                width: 1.0,
                color: palette.background.strong.color,
                ..Border::default()
            },
            ..Default::default()
        }
    }
    pub fn pane_focused(theme: &Theme) -> Style {
        let palette = theme.extended_palette();

        Style {
            background: Some(Color::BLACK.into()),
            border: Border {
                width: 1.0,
                color: palette.primary.strong.color,
                ..Border::default()
            },
            ..Default::default()
        }
    }
    pub fn sell_side_red(color_alpha: f32) -> Style {
        Style {
            text_color: Color::from_rgba(192.0 / 255.0, 80.0 / 255.0, 77.0 / 255.0, 1.0).into(),
            border: Border {
                width: 1.0,
                color: Color::from_rgba(192.0 / 255.0, 80.0 / 255.0, 77.0 / 255.0, color_alpha),
                ..Border::default()
            },
            ..Default::default()
        }
    }

    pub fn buy_side_green(color_alpha: f32) -> Style {
        Style {
            text_color: Color::from_rgba(81.0 / 255.0, 205.0 / 255.0, 160.0 / 255.0, 1.0).into(),
            border: Border {
                width: 1.0,
                color: Color::from_rgba(81.0 / 255.0, 205.0 / 255.0, 160.0 / 255.0, color_alpha),
                ..Border::default()
            },
            ..Default::default()
        }
    }
}