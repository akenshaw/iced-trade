#![windows_subsystem = "windows"]

mod data_providers;
use data_providers::{binance, bybit, Depth, Trade};
mod charts;
use charts::footprint::FootprintChart;
use charts::heatmap::HeatmapChart;
use charts::candlestick::CandlestickChart;
use charts::timeandsales::TimeAndSales;

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::vec;
use iced::{
    alignment, font, widget::{
        button, center, checkbox, mouse_area, opaque, pick_list, stack, text_input, tooltip, Column, Container, Row, Slider, Space, Text
    }, Alignment, Color, Task, Element, Font, Length, Renderer, Settings, Size, Subscription, Theme
};
use uuid::Uuid;

pub mod style;

use iced::widget::pane_grid::{self, PaneGrid, Configuration};
use iced::widget::{
    container, row, scrollable, text, responsive
};

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

    ChartUserUpdate(charts::Message, Uuid),

    // Market&User data stream
    UserKeySucceed(String),
    UserKeyError,
    TickerSelected(Ticker, Uuid),
    TimeframeSelected(Timeframe, pane_grid::Pane),
    ExchangeSelected(Exchange, Uuid),
    MarketWsEvent(MarketEvents),
    WsToggle,
    FetchEvent(Result<Vec<data_providers::Kline>, std::string::String>, PaneId, Timeframe),
    RestartStream(Option<pane_grid::Pane>, (Option<Ticker>, Option<Timeframe>, Option<f32>)),
    
    // Pane grid
    Split(pane_grid::Axis, pane_grid::Pane, Uuid),
    Clicked(pane_grid::Pane),
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    Maximize(pane_grid::Pane),
    Restore,
    Close(pane_grid::Pane),
    ToggleLayoutLock,

    // Modal
    OpenModal(pane_grid::Pane),
    CloseModal(Uuid),

    // Slider
    SliderChanged(PaneId, f32),
    SyncWithHeatmap(bool),

    CutTheKlineStream,

    ShowLayoutModal,
    HideLayoutModal,

    TicksizeSelected(TickMultiplier, Uuid),
    SetMinTickSize(f32, Uuid),
    
    ErrorOccurred(String),

    PaneContentSelected(String, Uuid, PaneStream),
}

struct State {
    dashboard: Dashboard,

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

    pane_streams: HashMap<Exchange, HashMap<Ticker, HashSet<StreamType>>>,
}

struct ChartState {
    candlestick_chart_a: Option<CandlestickChart>,
    candlestick_chart_b: Option<CandlestickChart>,
    heatmap_chart: Option<HeatmapChart>,
    footprint_chart: Option<FootprintChart>,
    time_and_sales: Option<TimeAndSales>,
}
impl Default for ChartState {
    fn default() -> Self {
        Self {
            candlestick_chart_a: None,
            candlestick_chart_b: None,
            heatmap_chart: None,
            footprint_chart: None,
            time_and_sales: None,
        }
    }
}

struct NetworkState {
    listen_key: Option<String>,
    binance_ws_state: BinanceWsState,
    bybit_ws_state: BybitWsState,
    user_ws_state: UserWsState,
    ws_running: bool,
    kline_stream: bool,
}

struct Dashboard {
    panes: pane_grid::State<PaneState>,
    show_layout_modal: bool,
    focus: Option<pane_grid::Pane>,
    first_pane: pane_grid::Pane,
    pane_lock: bool,
    pane_state_cache: HashMap<Uuid, (Option<Ticker>, Option<Timeframe>, Option<f32>)>,
    last_axis_split: Option<pane_grid::Axis>,
}
impl Dashboard {
    fn empty(pane_config: Configuration<PaneState>) -> Self {
        let panes: pane_grid::State<PaneState> = pane_grid::State::with_configuration(pane_config);
        let first_pane: pane_grid::Pane = *panes.panes.iter().next().unwrap().0;
        
        Self { 
            show_layout_modal: false,
            panes,
            focus: None,
            first_pane,
            pane_lock: false,
            pane_state_cache: HashMap::new(),
            last_axis_split: None,
        }
    }

    fn update_chart_state(&mut self, pane_id: Uuid, message: charts::Message) -> Result<(), &str> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                match pane_state.content {
                    PaneContent::Heatmap(ref mut chart) => {
                        chart.update(&message);

                        return Ok(());
                    },
                    PaneContent::Footprint(ref mut chart) => {
                        chart.update(&message);

                        return Ok(());
                    },
                    PaneContent::Candlestick(ref mut chart) => {
                        chart.update(&message);

                        return Ok(());
                    },
                    _ => {
                        return Err("No chart found");
                    }
                }
            }
        }
        Err("No pane found")
    }

    fn get_pane_stream_mut(&mut self, pane_id: Uuid) -> Result<&mut Vec<StreamType>, &str> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                return Ok(&mut pane_state.stream);
            }
        }
        Err("No pane found")
    }

    fn get_pane_settings_mut(&mut self, pane_id: Uuid) -> Result<&mut PaneSettings, &str> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                return Ok(&mut pane_state.settings);
            }
        }
        Err("No pane found")
    }

    fn set_pane_content(&mut self, pane_id: Uuid, content: PaneContent) -> Result<(), &str> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                pane_state.content = content;

                return Ok(());
            }
        }
        Err("No pane found")
    }

    fn footprint_change_ticksize(&mut self, pane_id: Uuid, new_tick_size: f32) -> Result<(), &str> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                match pane_state.content {
                    PaneContent::Footprint(ref mut chart) => {
                        chart.change_tick_size(new_tick_size);
                        
                        return Ok(());
                    },
                    _ => {
                        return Err("No footprint chart found");
                    }
                }
            }
        }
        Err("No pane found")
    }

    fn get_mutable_pane_settings(&mut self, pane: pane_grid::Pane) -> Result<&mut PaneSettings, &str> {
        self.panes.get_mut(pane).map(|pane_state| &mut pane_state.settings).ok_or("No pane found")
    }
    
    fn get_streams_vec(&self) -> Vec<&Vec<StreamType>> {
        self.panes.iter().map(|(_, pane_state)| &pane_state.stream).collect()
    }

    fn update_pane_data(&mut self, stream_type: StreamType, depth_update_t: i64, depth: Depth, trades_buffer: Vec<Trade>) {
        let depth = Rc::new(depth);

        let trades_buffer = trades_buffer.into_boxed_slice();

        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.matches_stream(&stream_type) {
                match &mut pane_state.content {
                    PaneContent::Heatmap(chart) => {
                        chart.insert_datapoint(&trades_buffer, depth_update_t, Rc::clone(&depth));
                    },
                    PaneContent::Footprint(chart) => {
                        chart.insert_datapoint(&trades_buffer, depth_update_t);
                    },
                    PaneContent::TimeAndSales(chart) => {
                        chart.update(&trades_buffer);
                    },
                    _ => {}
                }
            }
        }
    }
}

struct PaneState {
    id: Uuid,
    show_modal: bool,
    stream: Vec<StreamType>,
    content: PaneContent,
    settings: PaneSettings,
}

impl PaneState {
    fn new(id: Uuid, stream: Vec<StreamType>, settings: PaneSettings) -> Self {
        Self {
            id,
            show_modal: false,
            stream,
            content: PaneContent::Starter,
            settings,
        }
    }

    fn matches_stream(&self, stream_type: &StreamType) -> bool {
        self.stream.iter().any(|stream| stream == stream_type)
    }
}

enum PaneContent {
    Heatmap(HeatmapChart),
    Footprint(FootprintChart),
    Candlestick(CandlestickChart),
    TimeAndSales(TimeAndSales),
    Starter,
}

#[derive(Debug, Clone, Copy)]
struct PaneSettings {
    min_tick_size: Option<f32>,
    trade_size_filter: Option<f32>,
    tick_multiply: Option<TickMultiplier>,
    selected_ticker: Option<Ticker>,
    selected_exchange: Option<Exchange>,
    selected_timeframe: Option<Timeframe>,
}
impl Default for PaneSettings {
    fn default() -> Self {
        Self {
            min_tick_size: None,
            trade_size_filter: Some(0.0),
            tick_multiply: Some(TickMultiplier(10)),
            selected_ticker: None,
            selected_exchange: None,
            selected_timeframe: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PaneStream {
    exchange: Exchange,
    ticker: Ticker,
    timeframe: Option<Timeframe>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum StreamType {
    Kline(PaneStream),
    DepthAndTrades(PaneStream),
}


impl State {
    fn new() -> Self {
        let pane_config: Configuration<PaneState> = Configuration::Split {
            axis: pane_grid::Axis::Vertical,
            ratio: 0.8,
            a: Box::new(Configuration::Split {
                axis: pane_grid::Axis::Horizontal,
                ratio: 0.4,
                a: Box::new(Configuration::Split {
                    axis: pane_grid::Axis::Vertical,
                    ratio: 0.5,
                    a: Box::new(Configuration::Pane(
                        PaneState { 
                            id: Uuid::new_v4(), 
                            show_modal: false, 
                            stream: vec![],
                            content: PaneContent::Starter,
                            settings: PaneSettings {
                                min_tick_size: None,
                                trade_size_filter: None,
                                tick_multiply: None,
                                selected_ticker: None,
                                selected_exchange: None,
                                selected_timeframe: None,
                            },
                        })
                    ),
                    b: Box::new(Configuration::Pane(
                        PaneState { 
                            id: Uuid::new_v4(), 
                            show_modal: false, 
                            stream: vec![],
                            content: PaneContent::Starter,
                            settings: PaneSettings {
                                min_tick_size: None,
                                trade_size_filter: None,
                                tick_multiply: None,
                                selected_ticker: None,
                                selected_exchange: None,
                                selected_timeframe: None,
                            },
                        })
                    ),
                }),
                b: Box::new(Configuration::Split {
                    axis: pane_grid::Axis::Vertical,
                    ratio: 0.5,
                    a: Box::new(Configuration::Pane(
                        PaneState { 
                            id: Uuid::new_v4(), 
                            show_modal: false, 
                            stream: vec![],
                            content: PaneContent::Starter,
                            settings: PaneSettings {
                                min_tick_size: None,
                                trade_size_filter: None,
                                tick_multiply: None,
                                selected_ticker: None,
                                selected_exchange: None,
                                selected_timeframe: None,
                            },
                        })                      
                    ),
                    b: Box::new(Configuration::Pane(
                        PaneState { 
                            id: Uuid::new_v4(), 
                            show_modal: false, 
                            stream: vec![],
                            content: PaneContent::Starter,
                            settings: PaneSettings {
                                min_tick_size: None,
                                trade_size_filter: None,
                                tick_multiply: None,
                                selected_ticker: None,
                                selected_exchange: None,
                                selected_timeframe: None,
                            },
                        })
                    ),
                }),
            }),
            b: Box::new(Configuration::Pane(
                PaneState { 
                    id: Uuid::new_v4(), 
                    show_modal: false, 
                    stream: vec![],
                    content: PaneContent::Starter,
                    settings: PaneSettings {
                        min_tick_size: None,
                        trade_size_filter: None,
                        tick_multiply: None,
                        selected_ticker: None,
                        selected_exchange: None,
                        selected_timeframe: None,
                    },
                })
            ),
        };
        let dashboard = Dashboard::empty(pane_config);
        
        Self { 
            dashboard,
            kline_stream: true,
            listen_key: None,
            selected_ticker: None,
            selected_exchange: Some(Exchange::BinanceFutures),
            binance_ws_state: BinanceWsState::Disconnected,
            bybit_ws_state: BybitWsState::Disconnected,
            user_ws_state: UserWsState::Disconnected,
            ws_running: false,
            tick_multiply: TickMultiplier(10),
            min_tick_size: None, 
            exchange_latency: None,
            feed_latency_cache: VecDeque::new(),
            pane_streams: HashMap::new(),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ChartUserUpdate(message, pane_id) => {
                match self.dashboard.update_chart_state(pane_id, message) {
                    Ok(_) => Task::none(),
                    Err(err) => Task::none()
                }
            },
            Message::SetMinTickSize(min_tick_size, pane_id) => {      
                match self.dashboard.get_pane_settings_mut(pane_id) {
                    Ok(pane_settings) => {
                        pane_settings.min_tick_size = Some(min_tick_size);
                        Task::none()
                    },
                    Err(err) => Task::none()
                }
            },
            Message::TickerSelected(ticker, pane_id) => {
                let dashboard = &mut self.dashboard;

                match dashboard.get_pane_settings_mut(pane_id) {
                    Ok(pane_settings) => {
                        pane_settings.selected_ticker = Some(ticker);
                        
                        Task::none()
                    },
                    Err(err) => Task::none()
                }
            },
            Message::TicksizeSelected(tick_multiply, pane_id) => {
                match self.dashboard.get_pane_settings_mut(pane_id) {
                    Ok(pane_settings) => {
                        pane_settings.tick_multiply = Some(tick_multiply);

                        if let Some(min_tick_size) = pane_settings.min_tick_size {
                            match self.dashboard.footprint_change_ticksize(pane_id, tick_multiply.multiply_with_min_tick_size(min_tick_size)) {
                                Ok(_) => Task::none(),
                                Err(err) => Task::none()
                            }
                        } else {
                            Task::none()
                        }
                    },
                    Err(err) => Task::none()
                }
            },
            Message::TimeframeSelected(timeframe, pane) => {                
                match self.dashboard.get_mutable_pane_settings(pane) {
                    Ok(pane_settings) => {
                        pane_settings.selected_timeframe = Some(timeframe);

                        Task::none()
                    },
                    Err(err) => Task::none()
                }
            },
            Message::ExchangeSelected(exchange, pane) => {
                match self.dashboard.get_pane_settings_mut(pane) {
                    Ok(pane_settings) => {
                        pane_settings.selected_exchange = Some(exchange);

                        Task::none()
                    },
                    Err(err) => Task::none()
                }
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

                Task::none()
            },       
            Message::FetchEvent(klines, target_pane, timeframe) => {
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
                        binance::market_data::Event::DepthReceived(ticker, feed_latency, depth_update_t, depth, trades_buffer) => {                            
                            let stream_type = StreamType::DepthAndTrades(PaneStream {
                                exchange: Exchange::BinanceFutures,
                                ticker,
                                timeframe: None,
                            });
                            
                            self.dashboard.update_pane_data(stream_type, depth_update_t, depth, trades_buffer);
                        }
                        binance::market_data::Event::KlineReceived(ticker, kline, timeframe) => {
                        }
                    },
                    MarketEvents::Bybit(event) => match event {
                        bybit::market_data::Event::Connected(connection) => {
                            self.bybit_ws_state = BybitWsState::Connected(connection);
                        }
                        bybit::market_data::Event::Disconnected => {
                            self.bybit_ws_state = BybitWsState::Disconnected;
                        }
                        bybit::market_data::Event::DepthReceived(ticker, feed_latency, depth_update_t, depth, trades_buffer) => {
                            let stream_type = StreamType::DepthAndTrades(PaneStream {
                                exchange: Exchange::BybitLinear,
                                ticker,
                                timeframe: None,
                            });
                            
                            self.dashboard.update_pane_data(stream_type, depth_update_t, depth, trades_buffer);
                        }
                        bybit::market_data::Event::KlineReceived(kline, timeframe) => {
                        }
                    },
                }

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
                let cached_pane_state: (Option<Ticker>, Option<Timeframe>, Option<f32>) = *self.dashboard.pane_state_cache.get(&pane_id).unwrap_or(&(None, None, None));

                let new_pane = None;

                let focus_pane = if let Some((new_pane, _)) = 
                    self.dashboard.panes.split(axis, pane, PaneState::new(Uuid::new_v4(), vec![], PaneSettings::default())) {
                            Some(new_pane)
                        } else {
                            None
                        };

                if Some(focus_pane).is_some() {
                    self.dashboard.focus = focus_pane;
                }

                self.dashboard.last_axis_split = Some(axis);

                Task::perform(
                    async {
                    },
                    move |()| Message::RestartStream(new_pane, cached_pane_state)
                )
            },
            Message::Clicked(pane) => {
                self.dashboard.focus = Some(pane);
                Task::none()
            },
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.dashboard.panes.resize(split, ratio);
                Task::none()
            },
            Message::Dragged(pane_grid::DragEvent::Dropped {
                pane,
                target,
            }) => {
                self.dashboard.panes.drop(pane, target);
                Task::none()
            },
            Message::Dragged(_) => {
                Task::none()
            },
            Message::Maximize(pane) => {
                self.dashboard.panes.maximize(pane);
                Task::none()
            },
            Message::Restore => {
                self.dashboard.panes.restore();
                Task::none()
            },
            Message::Close(pane) => {       
                let pane_state = self.dashboard.panes.get(pane).unwrap();
                
                self.dashboard.pane_state_cache.insert(pane_state.id, (pane_state.settings.selected_ticker, pane_state.settings.selected_timeframe, pane_state.settings.min_tick_size));

                if let Some((_, sibling)) = self.dashboard.panes.close(pane) {
                    self.dashboard.focus = Some(sibling);
                }
                Task::none()
            },
            Message::ToggleLayoutLock => {
                self.dashboard.pane_lock = !self.dashboard.pane_lock;
                Task::none()
            },

            Message::Debug(_msg) => {
                Task::none()
            },

            Message::OpenModal(pane) => {
                if let Some(pane) = self.dashboard.panes.get_mut(pane) {
                    pane.show_modal = true;
                };
                Task::none()
            },
            Message::CloseModal(pane_id) => {
                for (_, pane_state) in self.dashboard.panes.iter_mut() {
                    if pane_state.id == pane_id {
                        pane_state.show_modal = false;
                    }
                }
                Task::none()
            },

            Message::SliderChanged(pane_id, value) => {

                Task::none()
            },
            Message::SyncWithHeatmap(sync) => {   
            
                Task::none()
            },
            Message::CutTheKlineStream => {
                self.kline_stream = true;
                Task::none()
            },

            Message::ShowLayoutModal => {
                self.dashboard.show_layout_modal = true;
                iced::widget::focus_next()
            },
            Message::HideLayoutModal => {
                self.dashboard.show_layout_modal = false;
                Task::none()
            },

            Message::ErrorOccurred(err) => {
                eprintln!("{err}");
                Task::none()
            },
            Message::PaneContentSelected(content, pane_id, pane_stream) => {
                match content.as_str() {
                    "Heatmap chart" => {
                        let pane_content = PaneContent::Heatmap(HeatmapChart::new());

                        match self.dashboard.get_pane_stream_mut(pane_id) {
                            Ok(vec_streams) => {
                                vec_streams.push(StreamType::DepthAndTrades(pane_stream));                        
                            },
                            Err(err) => {
                                dbg!("No pane found");
                            }
                        }

                        match self.dashboard.set_pane_content(pane_id, pane_content) {
                            Ok(_) => dbg!("Pane content set"),
                            Err(err) => dbg!("No pane found"),
                        };
                    },
                    "Footprint chart" => {
                        let footprint_chart = FootprintChart::new(1, 20.0, vec![], vec![]);
                        let pane_content = PaneContent::Footprint(footprint_chart);

                        match self.dashboard.get_pane_stream_mut(pane_id) {
                            Ok(vec_streams) => {
                                vec_streams.push(StreamType::DepthAndTrades(pane_stream));

                                vec_streams.push(StreamType::Kline(pane_stream));                             
                            },
                            Err(err) => {
                                dbg!("No pane found");
                            }
                        }

                        match self.dashboard.set_pane_content(pane_id, pane_content) {
                            Ok(_) => dbg!("Pane content set"),
                            Err(err) => dbg!("No pane found"),
                        };
                    },
                    "Candlestick chart" => {
                        let candlestick_chart = CandlestickChart::new(vec![], Timeframe::M1);
                        let pane_content = PaneContent::Candlestick(candlestick_chart);

                        match self.dashboard.get_pane_stream_mut(pane_id) {
                            Ok(vec_streams) => {
                                vec_streams.push(StreamType::Kline(pane_stream));
                            },
                            Err(err) => {
                                dbg!("No pane found");
                            }
                        }

                        match self.dashboard.set_pane_content(pane_id, pane_content) {
                            Ok(_) => dbg!("Pane content set"),
                            Err(err) => dbg!("No pane found"),
                        };
                    },
                    "Time&Sales" => {
                        let pane_content = PaneContent::TimeAndSales(TimeAndSales::new());

                        match self.dashboard.get_pane_stream_mut(pane_id) {
                            Ok(vec_streams) => {
                                vec_streams.push(StreamType::DepthAndTrades(pane_stream));
                            },
                            Err(err) => {
                                dbg!("No pane found");
                            }
                        }

                        match self.dashboard.set_pane_content(pane_id, pane_content) {
                            Ok(_) => dbg!("Pane content set"),
                            Err(err) => dbg!("No pane found"),
                        };
                    }
                    _ => {}
                }

                let streams_iter: Vec<&Vec<StreamType>> = self.dashboard.get_streams_vec();

                for stream_vec in streams_iter {
                    for stream in stream_vec {
                        match stream {
                            StreamType::Kline(pane_stream) | 
                            StreamType::DepthAndTrades(pane_stream) => {
                                self.pane_streams
                                    .entry(pane_stream.exchange)
                                    .or_insert_with(HashMap::new)
                                    .entry(pane_stream.ticker)
                                    .or_insert_with(HashSet::new)
                                    .insert(stream.clone());
                            }
                        }
                    }
                }

                dbg!(&self.pane_streams);

                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let focus = self.dashboard.focus;
        let total_panes = self.dashboard.panes.len();

        let pane_grid = PaneGrid::new(&self.dashboard.panes, |id, pane, is_maximized| {
            let is_focused = focus == Some(id);

            let chart_type = &self.dashboard.panes.get(id).unwrap().content;
    
            let content: pane_grid::Content<'_, Message, _, Renderer> = 
                pane_grid::Content::new(responsive(move |_| {
                    match chart_type {
                        PaneContent::Heatmap(chart) => view_chart(pane, chart),
                        
                        PaneContent::Footprint(chart) => view_chart(pane, chart),
                        
                        PaneContent::Candlestick(chart) => view_chart(pane, chart),

                        PaneContent::TimeAndSales(chart) => view_chart(pane, chart),

                        PaneContent::Starter => view_starter(pane),
                    }
                }));
    
            if self.dashboard.pane_lock {
                return content.style(style::pane_active);
            }
    
            let mut content = content.style(if is_focused {
                style::pane_focused
            } else {
                style::pane_active
            });

            let stream_name = pane.stream.iter().map(|stream| {
                match stream {
                    StreamType::Kline(pane_stream) => {
                        format!("{} {} {}", pane_stream.exchange, pane_stream.ticker, pane_stream.timeframe.unwrap_or_else(|| { dbg!("No timeframe found"); Timeframe::M1 }))
                    },
                    StreamType::DepthAndTrades(pane_stream) => {
                        format!("{} {}", pane_stream.exchange, pane_stream.ticker)
                    }
                }
            }).collect::<Vec<String>>().join(", ");
        
            if is_focused {
                let title_bar = pane_grid::TitleBar::new(Text::new(stream_name))
                    .always_show_controls()
                    .controls(view_controls(
                        id,
                        pane.id,
                        chart_type,
                        total_panes,
                        is_maximized,
                        &pane.settings,
                    ))
                    .padding(4)
                    .style(style::title_bar_focused);
                content = content.title_bar(title_bar);
            }
            content
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(6)
        .on_click(Message::Clicked)
        .on_drag(Message::Dragged)
        .on_resize(10, Message::Resized);

        let layout_lock_button = button(
            container(
                if self.dashboard.pane_lock { 
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

        let debug_button = button("Debug").on_press(Message::Debug("Debug".to_string()));

        let mut ws_controls = Row::new()
            .spacing(10)
            .align_items(Alignment::Center)
            .push(ws_button)
            .push(debug_button);

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
        }

        let content = Column::new()
            .padding(10)
            .spacing(10)
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

        if self.dashboard.show_layout_modal {
            let pane_to_split = self.dashboard.focus.unwrap_or_else(|| { dbg!("No focused pane found"); self.dashboard.first_pane });

            let mut axis_to_split = if rand::random() { pane_grid::Axis::Horizontal } else { pane_grid::Axis::Vertical };

            if let Some(axis) = self.dashboard.last_axis_split {
                if axis == pane_grid::Axis::Horizontal {
                    axis_to_split = pane_grid::Axis::Vertical;
                } else {
                    axis_to_split = pane_grid::Axis::Horizontal;
                }
            } 

            let add_pane_button = button("add new pane").width(iced::Pixels(200.0)).on_press(
                Message::Split(axis_to_split, pane_to_split, Uuid::new_v4())
            );

            let signup = container(
                Column::new()
                    .spacing(10)
                    .align_items(Alignment::Center)
                    .push(
                        Text::new("Add a new pane")
                            .size(20)
                    )
                    .push(add_pane_button)
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
            content 
                .into()
        }  
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = Vec::new();

        for (exchange, stream) in &self.pane_streams {
            match exchange {
                Exchange::BinanceFutures => {
                    let mut binance_streams = Vec::new();

                    for (_, stream_types) in stream {
                        for stream_type in stream_types {
                            match stream_type {
                                StreamType::Kline(pane_stream) => {
                                },
                                StreamType::DepthAndTrades(pane_stream) => {
                                    binance_streams.push(
                                        binance::market_data::connect_market_stream(*pane_stream)
                                            .map(|event: binance::market_data::Event| Message::MarketWsEvent(MarketEvents::Binance(event)))
                                    );
                                },
                            }
                        }
                    }

                    subscriptions.push(Subscription::batch(binance_streams));
                },
                Exchange::BybitLinear => {
                    let mut bybit_streams = Vec::new();

                    for (_, stream_types) in stream {
                        for stream_type in stream_types {
                            match stream_type {
                                StreamType::Kline(pane_stream) => {
                                },
                                StreamType::DepthAndTrades(pane_stream) => {
                                    bybit_streams.push(
                                        bybit::market_data::connect_market_stream(*pane_stream)
                                            .map(|event: bybit::market_data::Event| Message::MarketWsEvent(MarketEvents::Bybit(event)))
                                    );
                                },
                            }
                        }
                    }

                    subscriptions.push(Subscription::batch(bybit_streams));
                },
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
    fn view(&self, id: &PaneState) -> Element<Message>;
}

impl ChartView for HeatmapChart {
    fn view(&self, pane: &PaneState) -> Element<Message> {
        let underlay;

        let pane_id = pane.id;

        underlay = self.view().map(move |message| Message::ChartUserUpdate(message, pane_id));

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
                                .on_press(Message::CloseModal(pane_id))
                            )
                    )
            )
            .width(Length::Shrink)
            .padding(20)
            .max_width(500)
            .style(style::title_bar_active);

            return modal(underlay, signup, Message::CloseModal(pane_id));
        } else {
            underlay
        }
    }
}
impl ChartView for FootprintChart {
    fn view(&self, pane: &PaneState) -> Element<Message> {
        let pane_id = pane.id;

        self.view().map(move |message| Message::ChartUserUpdate(message, pane_id))
    }
}
impl ChartView for TimeAndSales {
    fn view(&self, pane: &PaneState) -> Element<Message> {
        let underlay;

        let pane_id = pane.id;

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
                                .on_press(Message::CloseModal(pane_id))
                            )
                    )
            )
            .width(Length::Shrink)
            .padding(20)
            .max_width(500)
            .style(style::title_bar_active);

            return modal(underlay, signup, Message::CloseModal(pane_id));
        } else {
            underlay
        }
    }
}
impl ChartView for CandlestickChart {
    fn view(&self, pane: &PaneState) -> Element<Message> {
        let pane_id = pane.id;

        self.view().map(move |message| Message::ChartUserUpdate(message, pane_id))
    }
}

fn view_chart<'a, C: ChartView>(
    pane: &'a PaneState,
    chart: &'a C,
) -> Element<'a, Message> {
    let chart_view: Element<Message> = chart.view(pane);

    let container = Container::new(chart_view)
        .width(Length::Fill)
        .height(Length::Fill);

    container.into()
}

fn view_starter<'a>(
    pane: &'a PaneState,
) -> Element<'a, Message> {
    let content_names = ["Heatmap chart", "Footprint chart", "Candlestick chart", "Time&Sales"];
    
    let content_selector = content_names.iter().fold(
        Column::new()
            .spacing(6)
            .align_items(Alignment::Center), |column, &label| {
                let mut btn = button(label).width(Length::Fill);
                if pane.settings.selected_ticker.is_some() && pane.settings.selected_exchange.is_some() {
                    let pane_stream = PaneStream {
                        exchange: pane.settings.selected_exchange.unwrap(),
                        ticker: pane.settings.selected_ticker.unwrap(),
                        timeframe: pane.settings.selected_timeframe,
                    };

                    btn = btn.on_press(
                        Message::PaneContentSelected(
                            label.to_string(), pane.id, pane_stream
                        )
                    );
                }
                column.push(btn)
            }
    );

    let symbol_selector = pick_list(
        &Ticker::ALL[..],
        pane.settings.selected_ticker,
        move |ticker| Message::TickerSelected(ticker, pane.id),
    ).placeholder("ticker...").text_size(13).width(Length::Fill);

    let exchange_selector = pick_list(
        &Exchange::ALL[..],
        pane.settings.selected_exchange,
        move |exchange| Message::ExchangeSelected(exchange, pane.id),
    ).placeholder("exchange...").text_size(13).width(Length::Fill);

    let picklists = Row::new()
        .spacing(6)
        .align_items(Alignment::Center)
        .push(exchange_selector)
        .push(symbol_selector);

    let column = Column::new()
        .padding(10)
        .spacing(10)
        .align_items(Alignment::Center)
        .push(picklists)
        .push(content_selector);
        
    let container = Container::new(
        Column::new()
            .spacing(10)
            .padding(20)
            .align_items(Alignment::Center)
            .max_width(300)
            .push(
                Text::new("Initialize the pane").size(16)
            )
            .push(scrollable(column))
        ).align_x(alignment::Horizontal::Center);
    
    container.into()
}

fn view_controls<'a>(
    pane: pane_grid::Pane,
    pane_id: Uuid,
    pane_type: &PaneContent,
    total_panes: usize,
    is_maximized: bool,
    settings: &PaneSettings,
) -> Element<'a, Message> {
    let mut row = row![].spacing(5);

    let (icon, message) = if is_maximized {
        (Icon::ResizeSmall, Message::Restore)
    } else {
        (Icon::ResizeFull, Message::Maximize(pane))
    };

    match pane_type {
        PaneContent::Heatmap(_) => {
        },
        PaneContent::TimeAndSales(_) => {
        },
        PaneContent::Footprint(_) => {
            let timeframe_picker = pick_list(
                &Timeframe::ALL[..],
                settings.selected_timeframe,
                move |timeframe| Message::TimeframeSelected(timeframe, pane),
            ).placeholder("Choose a timeframe...").text_size(11).width(iced::Pixels(80.0));
    
            let tf_tooltip = tooltip(timeframe_picker, "Timeframe", tooltip::Position::Top).style(style::tooltip);
    
            row = row.push(tf_tooltip);

            let ticksize_picker = pick_list(
                [TickMultiplier(1), TickMultiplier(2), TickMultiplier(5), TickMultiplier(10), TickMultiplier(25), TickMultiplier(50), TickMultiplier(100), TickMultiplier(200)],
                settings.tick_multiply, 
                move |tick_multiply| Message::TicksizeSelected(tick_multiply, pane_id)
            ).placeholder("Ticksize multiplier...").text_size(11).width(iced::Pixels(80.0));
            
            let ticksize_tooltip = tooltip(ticksize_picker, "Ticksize multiplier", tooltip::Position::Top).style(style::tooltip);
    
            row = row.push(ticksize_tooltip);
        },
        PaneContent::Candlestick(_) => {
            let timeframe_picker = pick_list(
                &Timeframe::ALL[..],
                settings.selected_timeframe,
                move |timeframe| Message::TimeframeSelected(timeframe, pane),
            ).placeholder("Choose a timeframe...").text_size(11).width(iced::Pixels(80.0));
    
            let tooltip = tooltip(timeframe_picker, "Timeframe", tooltip::Position::Top).style(style::tooltip);
    
            row = row.push(tooltip);
        },
        PaneContent::Starter => {
        },
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