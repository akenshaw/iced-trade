#![windows_subsystem = "windows"]

mod data_providers;
use data_providers::{binance, bybit};
mod charts;
use charts::footprint::FootprintChart;
use charts::heatmap::HeatmapChart;
use charts::candlestick::CandlestickChart;
use charts::timeandsales::TimeAndSales;

use std::collections::{VecDeque, HashMap};
use std::vec;
use iced::{
    alignment, font, widget::{
        button, center, checkbox, mouse_area, opaque, pick_list, stack, text_input, tooltip, Column, Container, Row, Slider, Space, Text
    }, Alignment, Color, Task, Element, Font, Length, Renderer, Settings, Size, Subscription, Theme
};

pub mod style;

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
    CandlestickChartA,
    CandlestickChartB,
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
    fn new(id: PaneId, from_cache: (Option<Ticker>, Option<Timeframe>, Option<f32>)) -> Self {
        Self { 
            id,
            show_modal: false,
            stream: from_cache,
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
    .run_with(move || State::new())
}

#[derive(Debug, Clone)]
pub enum MarketEvents {
    Binance(binance::market_data::Event),
    Bybit(bybit::market_data::Event),
}

#[derive(Debug, Clone)]
pub enum Message {
    Debug(String),

    Candlestick(charts::Message, PaneId),
    Heatmap(charts::Message),
    Footprint(charts::Message),

    // Market&User data stream
    UserKeySucceed(String),
    UserKeyError,
    TickerSelected(Ticker),
    TimeframeSelected(Timeframe, pane_grid::Pane),
    ExchangeSelected(Exchange),
    MarketWsEvent(MarketEvents),
    WsToggle,
    FetchEvent(Result<Vec<data_providers::Kline>, std::string::String>, PaneId, Timeframe),
    RestartStream(Option<pane_grid::Pane>, (Option<Ticker>, Option<Timeframe>, Option<f32>)),
    
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
    CloseModal(PaneId),

    // Slider
    SliderChanged(PaneId, f32),
    SyncWithHeatmap(bool),

    CutTheKlineStream,

    ShowLayoutModal,
    HideLayoutModal,

    TicksizeSelected(TickMultiplier),
    SetMinTickSize(f32),
    
    ErrorOccurred(String),
}

struct State {
    candlestick_chart_a: Option<CandlestickChart>,
    time_and_sales: Option<TimeAndSales>,
    candlestick_chart_b: Option<CandlestickChart>,
    heatmap_chart: Option<HeatmapChart>,
    footprint_chart: Option<FootprintChart>,

    exchange_latency: Option<(u32, u32)>,

    tick_multiply: TickMultiplier,
    min_tick_size: Option<f32>,

    // data streams
    listen_key: Option<String>,
    selected_ticker: Option<Ticker>,
    selected_exchange: Option<Exchange>,

    binance_ws_state: BinanceWsState,
    bybit_ws_state: BybitWsState,

    user_ws_state: UserWsState,

    ws_running: bool,

    kline_stream: bool,

    feed_latency_cache: VecDeque<data_providers::FeedLatency>, 

    // pane grid
    panes: pane_grid::State<PaneSpec>,
    focus: Option<pane_grid::Pane>,
    first_pane: pane_grid::Pane,
    pane_lock: bool,

    pane_state_cache: HashMap<PaneId, (Option<Ticker>, Option<Timeframe>, Option<f32>)>,

    last_axis_split: Option<pane_grid::Axis>,  

    show_layout_modal: bool,  
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
                            id: PaneId::CandlestickChartA, 
                            show_modal: false, 
                            stream: (Some(Ticker::BTCUSDT), Some(Timeframe::M15), None)
                        })
                    ),
                    b: Box::new(Configuration::Pane(
                        PaneSpec { 
                            id: PaneId::CandlestickChartB, 
                            show_modal: false, 
                            stream: (Some(Ticker::BTCUSDT), Some(Timeframe::M1), None)
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
            kline_stream: true,
            candlestick_chart_a: None,
            time_and_sales: None,
            candlestick_chart_b: None,
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
            tick_multiply: TickMultiplier(10),
            min_tick_size: None, 
            exchange_latency: None,
            feed_latency_cache: VecDeque::new(),
            pane_state_cache: HashMap::new(),
            last_axis_split: None,
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Candlestick(message, pane_id) => {
                if pane_id == PaneId::CandlestickChartA {
                    if let Some(chart) = &mut self.candlestick_chart_a {
                        chart.update(&message);
                    }
                } else if pane_id == PaneId::CandlestickChartB {
                    if let Some(chart) = &mut self.candlestick_chart_b {
                        chart.update(&message);
                    }
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

                if let Some(footprint_chart) = &mut self.footprint_chart {
                    let tick_size = self.tick_multiply.multiply_with_min_tick_size(self.min_tick_size.unwrap_or(1.0));
                    footprint_chart.change_tick_size(tick_size);
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
                if let Some(footprint_chart) = &mut self.footprint_chart {
                    let tick_size = tick_multiply.multiply_with_min_tick_size(self.min_tick_size.unwrap_or(1.0));
                    footprint_chart.change_tick_size(tick_size);
        
                    self.tick_multiply = tick_multiply;
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
                                binance::market_data::fetch_klines(*selected_ticker, timeframe)
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
                                            let binance_klines: Vec<data_providers::Kline> = klines.iter().map(|kline| {
                                                data_providers::Kline {
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
            Message::RestartStream(pane, cached_state) => {
                if let Some(pane) = pane {
                    if let Some(timeframe) = cached_state.1 {
                        Task::perform(
                            async {
                            },
                            move |()| Message::TimeframeSelected(timeframe, pane)
                        )
                    } else {
                        Task::perform(
                            async {
                            },
                            move |()| Message::ErrorOccurred(format!("No timeframe found in pane state to stream"))
                        )
                    }
                } else {
                    Task::none()
                }
            },
            Message::WsToggle => {
                self.ws_running = !self.ws_running;

                if self.ws_running {  
                    let mut tasks: Vec<Task<Message>> = vec![];
        
                    for (_, pane_state) in self.panes.iter() {
                        if pane_state.id == PaneId::HeatmapChart {
                            self.heatmap_chart = Some(HeatmapChart::new());
                        }
                        if pane_state.id == PaneId::TimeAndSales {
                            self.time_and_sales = Some(TimeAndSales::new());
                        }
                        if pane_state.id == PaneId::FootprintChart {
                            match self.selected_exchange {
                                Some(Exchange::BinanceFutures) => {
                                    let fetch_ticksize: Task<Message> = Task::perform(
                                        binance::market_data::fetch_ticksize(self.selected_ticker.unwrap_or(Ticker::BTCUSDT)),
                                        move |result| match result {
                                            Ok(ticksize) => Message::SetMinTickSize(ticksize),
                                            Err(err) => {
                                                Message::ErrorOccurred(err.to_string())
                                            }
                                        }
                                    );
                                    tasks.push(fetch_ticksize);
                                },
                                Some(Exchange::BybitLinear) => {
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
                                },
                                None => {
                                    eprintln!("No exchange selected");
                                }
                            }
                        }

                        if let Some(selected_timeframe) = pane_state.stream.1 {

                            let pane_id: PaneId = pane_state.id;

                            match self.selected_exchange {
                                Some(Exchange::BinanceFutures) => {
                                    let fetch_klines: Task<Message> = Task::perform(
                                        binance::market_data::fetch_klines(self.selected_ticker.unwrap_or(Ticker::BTCUSDT), selected_timeframe)
                                            .map_err(|err| format!("{err}")), 
                                        move |klines: Result<Vec<data_providers::Kline>, String>| {
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
                                                    let binance_klines: Vec<data_providers::Kline> = klines.iter().map(|kline| {
                                                        data_providers::Kline {
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
                    self.time_and_sales = None;
                    self.candlestick_chart_a = None;
                    self.candlestick_chart_b = None;
                    self.footprint_chart = None;

                    self.exchange_latency = None;
                    self.feed_latency_cache.clear();

                    Task::none()
                }
            },       
            Message::FetchEvent(klines, target_pane, timeframe) => {
                match klines {
                    Ok(klines) => {
                        match target_pane {
                            PaneId::CandlestickChartA => {
                                self.candlestick_chart_a = Some(CandlestickChart::new(klines, timeframe));
                            },
                            PaneId::CandlestickChartB => {
                                self.candlestick_chart_b = Some(CandlestickChart::new(klines, timeframe));
                            },
                            PaneId::FootprintChart => {
                                if let Some(heatmap_chart) = &mut self.heatmap_chart {
                                    let copied_trades: Vec<data_providers::Trade> = heatmap_chart.get_raw_trades();

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

                                    let tick_size = self.tick_multiply.multiply_with_min_tick_size(self.min_tick_size.unwrap_or(1.0));

                                    self.footprint_chart = Some(FootprintChart::new(timeframe_u16, tick_size, klines_raw, copied_trades));
                                }
                            },
                            _ => {}
                        }
                    },
                    Err(err) => {
                        eprintln!("Error fetching klines: {err}");
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
                        binance::market_data::Event::DepthReceived(feed_latency, depth_update, depth, trades_buffer) => {
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

                            self.feed_latency_cache.push_back(feed_latency);
                        }
                        binance::market_data::Event::KlineReceived(kline, timeframe) => {
                            for (_, pane_state) in self.panes.iter() {
                                if let Some(selected_timeframe) = pane_state.stream.1 {
                                    if selected_timeframe == timeframe {
                                        match pane_state.id {
                                            PaneId::CandlestickChartA => {
                                                if let Some(chart) = &mut self.candlestick_chart_a {
                                                    chart.insert_datapoint(&kline);
                                                }
                                            },
                                            PaneId::CandlestickChartB => {
                                                if let Some(chart) = &mut self.candlestick_chart_b {
                                                    chart.insert_datapoint(&kline);
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

                            self.update_exchange_latency();
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
                        bybit::market_data::Event::DepthReceived(feed_latency, depth_update, depth, trades_buffer) => {
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

                            self.feed_latency_cache.push_back(feed_latency);
                        }
                        bybit::market_data::Event::KlineReceived(kline, timeframe) => {
                            for (_, pane_state) in self.panes.iter() {
                                if let Some(selected_timeframe) = pane_state.stream.1 {
                                    if selected_timeframe == timeframe {
                                        let binance_kline = data_providers::Kline {
                                            time: kline.time,
                                            open: kline.open,
                                            high: kline.high,
                                            low: kline.low,
                                            close: kline.close,
                                            volume: kline.volume,
                                            taker_buy_base_asset_volume: -1.0,
                                        };

                                        match pane_state.id {
                                            PaneId::CandlestickChartA => {
                                                if let Some(chart) = &mut self.candlestick_chart_a {
                                                    chart.insert_datapoint(&binance_kline);
                                                }
                                            },
                                            PaneId::CandlestickChartB => {
                                                if let Some(chart) = &mut self.candlestick_chart_b {
                                                    chart.insert_datapoint(&binance_kline);
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

                            self.update_exchange_latency();
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
                let cached_pane_state: (Option<Ticker>, Option<Timeframe>, Option<f32>) = *self.pane_state_cache.get(&pane_id).unwrap_or(&(None, None, None));

                let new_pane = None;

                let focus_pane = if let Some((new_pane, _)) = self.panes.split(axis, pane, PaneSpec::new(pane_id, cached_pane_state)) {
                    Some(new_pane)
                } else if let Some((&first_pane, _)) = self.panes.panes.iter().next() {
                    self.panes.split(axis, first_pane, PaneSpec::new(pane_id, cached_pane_state)).map(|(new_pane, _)| new_pane)
                } else {
                    None
                };

                if Some(focus_pane).is_some() {
                    self.focus = focus_pane;
                }

                self.last_axis_split = Some(axis);

                Task::perform(
                    async {
                    },
                    move |()| Message::RestartStream(new_pane, cached_pane_state)
                )
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
                let pane_state = self.panes.get(pane).unwrap();
                
                self.pane_state_cache.insert(pane_state.id, (pane_state.stream.0, pane_state.stream.1, pane_state.stream.2));

                if let Some((_, sibling)) = self.panes.close(pane) {
                    self.focus = Some(sibling);
                }
                Task::none()
            },
            Message::ToggleLayoutLock => {
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
            Message::CloseModal(pane_id) => {
                for pane in self.panes.panes.values_mut() {
                    if pane.id == pane_id {
                        pane.show_modal = false;
                    }
                }
                Task::none()
            },

            Message::SliderChanged(pane_id, value) => {
                if pane_id == PaneId::TimeAndSales {
                    if let Some(time_and_sales) = &mut self.time_and_sales {
                        time_and_sales.set_size_filter(value);

                        if let Some(heatmap_chart) = &mut self.heatmap_chart {
                            let sync_filter_heatmap = time_and_sales.get_filter_sync_heatmap();

                            if sync_filter_heatmap {
                                heatmap_chart.set_size_filter(value);
                            }
                        }
                    }
                } else if pane_id == PaneId::HeatmapChart {
                    if let Some(heatmap_chart) = &mut self.heatmap_chart {
                        heatmap_chart.set_size_filter(value);

                        if let Some(time_and_sales) = &mut self.time_and_sales {
                            time_and_sales.set_filter_sync_heatmap(false)
                        }
                    }
                }

                Task::none()
            },
            Message::SyncWithHeatmap(sync) => {                
                if let Some(time_and_sales) = &mut self.time_and_sales {
                    time_and_sales.set_filter_sync_heatmap(sync);
                }
    
                if sync {
                    if let Some(time_and_sales) = &mut self.time_and_sales {
                        let size_filter_time_and_sales = time_and_sales.get_size_filter();

                        if let Some(heatmap_chart) = &mut self.heatmap_chart {
                            heatmap_chart.set_size_filter(size_filter_time_and_sales);
                        }
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
    
            let content: pane_grid::Content<'_, Message, _, Renderer> = 
                pane_grid::Content::new(responsive(move |_| {
                    match pane.id {
                        PaneId::HeatmapChart => view_content(
                            pane,
                            self.heatmap_chart.as_ref()
                        ),
                        PaneId::FootprintChart => view_content(
                            pane,
                            self.footprint_chart.as_ref()
                        ),
                        PaneId::CandlestickChartA => view_content(
                            pane,
                            self.candlestick_chart_a.as_ref()
                        ),
                        PaneId::CandlestickChartB => view_content(
                            pane,
                            self.candlestick_chart_b.as_ref()
                        ),
                        PaneId::TimeAndSales => view_content(
                            pane,
                            self.time_and_sales.as_ref()
                        ),
                        _ => Column::new().into(),
                    }
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
                    PaneId::CandlestickChartA => "Candlestick",
                    PaneId::CandlestickChartB => "Candlestick",
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
            let exchange_latency_tooltip: String;
            let mut highest_latency: i32 = 0;

            if let Some((depth_latency, trade_latency)) = self.exchange_latency {
                exchange_latency_tooltip = format!(
                    "Feed Latencies\n->Depth: {depth_latency} ms\n->Trade: {trade_latency} ms",
                );

                highest_latency = std::cmp::max(depth_latency as i32, trade_latency as i32);
            } else {
                exchange_latency_tooltip = "No latency data".to_string();

                highest_latency = 0;
            }

            let exchange_latency_tooltip = Text::new(exchange_latency_tooltip).size(12);

            let latency_emoji: &str = if highest_latency > 250 {
                "🔴"
            } else if highest_latency > 100 {
                "🟠"
            } else {
                "🟢"
            };
                
            let exchange_info = Row::new()
                .spacing(5)
                .align_items(Alignment::Center)
                .push(
                    Text::new(latency_emoji)
                        .shaping(text::Shaping::Advanced).size(8)
                )
                .push(
                    Column::new()
                        .align_items(Alignment::Start)
                        .push(
                            Text::new(self.selected_exchange.unwrap_or_else(|| { dbg!("No exchange found"); Exchange::BinanceFutures }).to_string()).size(10)
                        )
                        .push(
                            Text::new(format!("{} ms", highest_latency)).size(10)
                        )
                );
            
            ws_controls = ws_controls.push(
                Row::new()
                    .spacing(10)
                    .align_items(Alignment::Center)
                    .push(tooltip(exchange_info, exchange_latency_tooltip, tooltip::Position::Bottom).style(style::tooltip))
                    .push(
                        Text::new(self.selected_ticker.unwrap_or_else(|| { dbg!("No ticker found"); Ticker::BTCUSDT }).to_string()).size(20)
                    )
            );
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
                    .push(layout_controls)
            )
            .push(pane_grid);

        if self.show_layout_modal {
            let mut buttons = Column::new().spacing(2).align_items(Alignment::Start);

            let pane_info = vec![
                (PaneId::HeatmapChart, "Heatmap Chart"),
                (PaneId::FootprintChart, "Footprint Chart"),
                (PaneId::CandlestickChartA, "Candlestick Chart 1"),
                (PaneId::CandlestickChartB, "Candlestick Chart 2"),
                (PaneId::TimeAndSales, "Time & Sales"),
            ];

            let pane_to_split = self.focus.unwrap_or_else(|| { dbg!("No focused pane found"); self.first_pane });

            let mut axis_to_split = if rand::random() { pane_grid::Axis::Horizontal } else { pane_grid::Axis::Vertical };

            if let Some(axis) = self.last_axis_split {
                if axis == pane_grid::Axis::Horizontal {
                    axis_to_split = pane_grid::Axis::Vertical;
                } else {
                    axis_to_split = pane_grid::Axis::Horizontal;
                }
            } 

            for (pane_id, label) in pane_info {
                let button = button(label).width(iced::Pixels(200.0));

                if self.panes.iter().any(|(_, ps)| ps.id == pane_id) {
                    buttons = buttons.push(button);
                } else {
                    let message = Message::Split(axis_to_split, pane_to_split, pane_id);
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

    fn update_exchange_latency(&mut self) {
        let mut depth_latency_sum: i64 = 0;
        let mut depth_latency_count: i64 = 0;
        let mut trade_latency_sum: i64 = 0;
        let mut trade_latency_count: i64 = 0;

        for feed_latency in self.feed_latency_cache.iter() {
            depth_latency_sum += feed_latency.depth_latency;
            depth_latency_count += 1;

            if let Some(trade_latency) = feed_latency.trade_latency {
                trade_latency_sum += trade_latency;
                trade_latency_count += 1;
            }
        }

        let average_depth_latency: Option<i64> = if depth_latency_count > 0 {
            Some(depth_latency_sum / depth_latency_count)
        } else {
            None
        };

        let average_trade_latency: Option<i64> = if trade_latency_count > 0 {
            Some(trade_latency_sum / trade_latency_count)
        } else {
            None
        };

        if let (Some(average_depth_latency), Some(average_trade_latency)) = (average_depth_latency, average_trade_latency) {
            self.exchange_latency = Some((average_depth_latency as u32, average_trade_latency as u32));
        }

        while self.feed_latency_cache.len() > 100 {
            self.feed_latency_cache.pop_front();
        }
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


trait ChartView {
    fn view(&self, id: &PaneSpec) -> Element<Message>;
}

impl ChartView for HeatmapChart {
    fn view(&self, pane: &PaneSpec) -> Element<Message> {
        let underlay;

        underlay = self.view().map(Message::Heatmap);

        if pane.show_modal {
            let size_filter = &self.get_size_filter();

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
                                Slider::new(0.0..=50000.0, *size_filter, move |value| Message::SliderChanged(PaneId::HeatmapChart, value))
                                    .step(500.0)
                            )
                            .push(
                                Text::new(format!("${size_filter}")).size(16)
                            )
                    )
                    .push( 
                        Row::new()
                            .spacing(10)
                            .push(
                                button("Close")
                                .on_press(Message::CloseModal(pane.id))
                            )
                    )
            )
            .width(Length::Shrink)
            .padding(20)
            .max_width(500)
            .style(style::title_bar_active);

            return modal(underlay, signup, Message::CloseModal(pane.id));
        } else {
            underlay
        }
    }
}
impl ChartView for FootprintChart {
    fn view(&self, _paneid: &PaneSpec) -> Element<Message> {
        self.view().map(Message::Footprint)
    }
}
impl ChartView for TimeAndSales {
    fn view(&self, pane: &PaneSpec) -> Element<Message> {
        let underlay;

        underlay = self.view();

        if pane.show_modal {
            let size_filter = &self.get_size_filter();

            let filter_sync_heatmap = &self.get_filter_sync_heatmap();

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
                                Slider::new(0.0..=50000.0, *size_filter, move |value| Message::SliderChanged(PaneId::TimeAndSales, value))
                                    .step(500.0)
                            )
                            .push(
                                Text::new(format!("${size_filter}")).size(16)
                            )
                            .push(
                                checkbox("Sync Heatmap with", *filter_sync_heatmap)
                                    .on_toggle(Message::SyncWithHeatmap)
                            )
                    )
                    .push( 
                        Row::new()
                            .spacing(10)
                            .push(
                                button("Close")
                                .on_press(Message::CloseModal(pane.id))
                            )
                    )
            )
            .width(Length::Shrink)
            .padding(20)
            .max_width(500)
            .style(style::title_bar_active);

            return modal(underlay, signup, Message::CloseModal(pane.id));
        } else {
            underlay
        }
    }
}
impl ChartView for CandlestickChart {
    fn view(&self, pane: &PaneSpec) -> Element<Message> {
        let pane_id = pane.id;

        self.view().map(move |message| Message::Candlestick(message, pane_id))
    }
}

fn view_content<'a, C: ChartView>(
    pane: &'a PaneSpec,
    chart: Option<&'a C>
) -> Element<'a, Message> {
    let chart_view: Element<Message> = match chart {
        Some(chart) => chart.view(pane),
        None => Text::new("No data")
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    };

    let container = Container::new(chart_view)
        .width(Length::Fill)
        .height(Length::Fill);

    container.into()
}

fn view_controls<'a>(
    pane: pane_grid::Pane,
    pane_id: PaneId,
    total_panes: usize,
    is_maximized: bool,
    selected_timeframe: Option<&'a Timeframe>,
    selected_ticksize: TickMultiplier,
) -> Element<'a, Message> {
    let mut row = row![].spacing(5);

    let (icon, message) = if is_maximized {
        (Icon::ResizeSmall, Message::Restore)
    } else {
        (Icon::ResizeFull, Message::Maximize(pane))
    };

    if pane_id == PaneId::CandlestickChartA || pane_id == PaneId::CandlestickChartB || pane_id == PaneId::FootprintChart {
        let timeframe_picker = pick_list(
            &Timeframe::ALL[..],
            selected_timeframe,
            move |timeframe| Message::TimeframeSelected(timeframe, pane),
        ).placeholder("Choose a timeframe...").text_size(11).width(iced::Pixels(80.0));

        let tooltip = tooltip(timeframe_picker, "Timeframe", tooltip::Position::Top).style(style::tooltip);

        row = row.push(tooltip);
    }
    if pane_id == PaneId::FootprintChart {
        let ticksize_picker = pick_list(
            [TickMultiplier(1), TickMultiplier(2), TickMultiplier(5), TickMultiplier(10), TickMultiplier(25), TickMultiplier(50), TickMultiplier(100), TickMultiplier(200)],
            Some(selected_ticksize), 
            Message::TicksizeSelected,
        ).placeholder("Ticksize multiplier...").text_size(11).width(iced::Pixels(80.0));
        let tooltip = tooltip(ticksize_picker, "Ticksize multiplier", tooltip::Position::Top).style(style::tooltip);

        row = row.push(tooltip);
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickMultiplier(u16);

impl std::fmt::Display for TickMultiplier {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}x", self.0)
    }
}

impl TickMultiplier {
    fn multiply_with_min_tick_size(&self, min_tick_size: f32) -> f32 {
        self.0 as f32 * min_tick_size
    }
}