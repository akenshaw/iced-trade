#![windows_subsystem = "windows"]

mod data_providers;
use data_providers::binance::{user_data, market_data};
mod charts;
use charts::custom_line::{self, CustomLine};
use charts::heatmap::{self, Heatmap};
use hmac::digest::consts::False;
use hmac::digest::CtOutput;

use std::{sync::Arc, vec};
use std::cell::RefCell;
use chrono::{NaiveDateTime, DateTime, Utc};
use iced::{
    advanced::widget, alignment, executor, font, theme::{self, Custom}, widget::{
        button, center, checkbox, mouse_area, opaque, pick_list, stack, text_input, tooltip, Column, Container, Row, Slider, Space, Text
    }, Alignment, Color, Command, Element, Font, Length, Renderer, Settings, Size, Subscription, Theme
};
use iced::advanced::Application;

use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    container, row, scrollable, text, responsive
};
use futures::TryFutureExt;
use plotters_iced::sample::lttb::DataPoint;

use std::collections::HashMap;

struct Wrapper<'a>(&'a DateTime<Utc>, &'a f32);
impl DataPoint for Wrapper<'_> {
    #[inline]
    fn x(&self) -> f64 {
        self.0.timestamp() as f64
    }
    #[inline]
    fn y(&self) -> f64 {
        *self.1 as f64
    }
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
enum WsState {
    Connected(market_data::Connection),
    Disconnected,
}
impl Default for WsState {
    fn default() -> Self {
        Self::Disconnected
    }
}

enum UserWsState {
    Connected(user_data::Connection),
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
    CandlestickChart,
    CustomChart,
    TimeAndSales,
    TradePanel,
}

#[derive(Debug, Clone, Copy)]
struct Pane {
    id: PaneId,
    show_modal: bool,
}

impl Pane {
    fn new(id: PaneId) -> Self {
        Self { 
            id,
            show_modal: false,
        }
    }
}
#[derive(Debug, Clone, Copy)]
enum StreamType {
    Klines(Ticker, Timeframe),
    DepthAndTrades(Ticker),
    UserStream,
}

fn main() {
    State::run(Settings {
        antialiasing: true,
        ..Settings::default()
    })
    .unwrap();
}

#[derive(Debug, Clone)]
pub enum Message {
    Debug(String),

    CustomLine(custom_line::Message),
    Candlestick(custom_line::Message),
    Heatmap(heatmap::Message),

    // Market&User data stream
    UserKeySucceed(String),
    UserKeyError,
    TickerSelected(Ticker),
    TimeframeSelected(Timeframe, pane_grid::Pane),
    ExchangeSelected(&'static str),
    MarketWsEvent(market_data::Event),
    WsToggle(),
    FetchEvent(Result<Vec<market_data::Kline>, std::string::String>, PaneId),
    
    // Pane grid
    Split(pane_grid::Axis, pane_grid::Pane, PaneId),
    Clicked(pane_grid::Pane),
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    Maximize(pane_grid::Pane),
    Restore,
    Close(pane_grid::Pane),
    ToggleLayoutLock,

    // Font
    FontLoaded(Result<(), font::Error>),

    // Modal
    OpenModal(pane_grid::Pane),
    CloseModal,

    // Slider
    SliderChanged(PaneId, f32),
    SyncWithHeatmap(bool),

    CutTheKlineStream,

    ShowLayoutModal,
    HideLayoutModal,
}

struct State {
    show_layout_modal: bool,

    candlestick_chart: Option<CustomLine>,
    time_and_sales: Option<TimeAndSales>,
    custom_line: Option<CustomLine>,
    heatmap_chart: Option<Heatmap>,

    // data streams
    listen_key: Option<String>,
    selected_ticker: Option<Ticker>,
    selected_timeframe: Option<Timeframe>,
    selected_exchange: Option<&'static str>,
    ws_state: WsState,
    user_ws_state: UserWsState,
    ws_running: bool,

    // pane grid
    panes_open: HashMap<PaneId, (bool, Option<StreamType>)>,
    panes: pane_grid::State<Pane>,
    focus: Option<pane_grid::Pane>,
    first_pane: pane_grid::Pane,
    pane_lock: bool,

    size_filter_timesales: f32,
    size_filter_heatmap: f32,
    sync_heatmap: bool,

    kline_stream: bool,
}

impl Application for State {
    type Renderer = Renderer;
    type Message = self::Message;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let (panes, first_pane) = pane_grid::State::new(Pane::new(PaneId::CustomChart));

        let mut panes_open: HashMap<PaneId, (bool, Option<StreamType>)> = HashMap::new();
        panes_open.insert(PaneId::HeatmapChart, (true, Some(StreamType::DepthAndTrades(Ticker::BTCUSDT))));
        panes_open.insert(PaneId::TimeAndSales, (false, None));
        panes_open.insert(PaneId::CandlestickChart, (false, Some(StreamType::Klines(Ticker::BTCUSDT, Timeframe::M15))));
        panes_open.insert(PaneId::CustomChart, (true, Some(StreamType::Klines(Ticker::BTCUSDT, Timeframe::M1))));
        panes_open.insert(PaneId::TradePanel, (false, None));
        (
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
                listen_key: None,
                selected_ticker: None,
                selected_timeframe: Some(Timeframe::M1),
                selected_exchange: Some("Binance Futures"),
                ws_state: WsState::Disconnected,
                user_ws_state: UserWsState::Disconnected,
                ws_running: false,
                panes,
                panes_open,
                focus: None,
                first_pane,
                pane_lock: false,
            },
            Command::batch(vec![
                font::load(ICON_BYTES).map(Message::FontLoaded),

                if API_KEY != "" && SECRET_KEY != "" {
                    Command::perform(user_data::get_listen_key(API_KEY, SECRET_KEY), |res| {
                        match res {
                            Ok(listen_key) => {
                                Message::UserKeySucceed(listen_key)
                            },
                            Err(err) => {
                                dbg!(err);
                                Message::UserKeyError
                            }
                        }
                    })
                } else {
                    eprintln!("API keys not set");
                    Command::none()
                },
                Command::perform(
                    async move {
                        (pane_grid::Axis::Horizontal, first_pane) 
                    },
                    move |(axis, pane)| {
                        Message::Split(axis, pane, PaneId::HeatmapChart)
                    }
                ),
            ]),
        )
    }

    fn title(&self) -> String {
        "Iced Trade".to_owned()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::CustomLine(message) => {
                if let Some(custom_line) = &mut self.custom_line {
                    custom_line.update(message);
                }
                Command::none()
            },
            Message::Candlestick(message) => {
                if let Some(candlesticks) = &mut self.candlestick_chart {
                    candlesticks.update(message);
                }
                Command::none()
            },
            Message::Heatmap(message) => {
                if let Some(heatmap) = &mut self.heatmap_chart {
                    heatmap.update(message);
                }
                Command::none()
            },

            Message::TickerSelected(ticker) => {
                self.selected_ticker = Some(ticker.clone());

                for value in self.panes_open.values_mut() {
                    if let (true, Some(StreamType::Klines(_, timeframe))) = value {
                        *value = (true, Some(StreamType::Klines(ticker.clone(), *timeframe)));
                    }
                }

                Command::none()
            },
            Message::TimeframeSelected(timeframe, pane) => {
                if !self.ws_running {
                    return Command::none();
                }

                let selected_ticker = match &self.selected_ticker {
                    Some(ticker) => ticker,
                    None => {
                        eprintln!("No ticker selected");
                        return Command::none();
                    }
                };
                self.kline_stream = false;
                
                let mut commands = vec![];
                let mut dropped_streams = vec![];

                self.panes.panes.get(&pane).map(|pane| {
                    if let Some((_, stream_type)) = self.panes_open.get(&pane.id) {
                        if let Some(StreamType::Klines(ticker, _)) = stream_type {
                            self.panes_open.insert(pane.id, (true, Some(StreamType::Klines(*ticker, timeframe))));   

                            let pane_id = pane.id;
                            let fetch_klines = Command::perform(
                            market_data::fetch_klines(*selected_ticker, timeframe)
                                .map_err(|err| format!("{}", err)), 
                            move |klines| {
                                Message::FetchEvent(klines, pane_id)
                            });

                            dropped_streams.push(pane_id);
                            
                            commands.push(fetch_klines);                                  
                        };
                    }
                });
        
                // sleep to drop existent stream and create new one
                let remove_active_stream = Command::perform(
                    async {
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    },
                    move |_| Message::CutTheKlineStream
                );
                commands.push(remove_active_stream);

                Command::batch(commands)
            },
            Message::ExchangeSelected(exchange) => {
                self.selected_exchange = Some(exchange);
                Command::none()
            },
            Message::WsToggle() => {
                self.ws_running =! self.ws_running;

                if self.ws_running {  
                    let mut commands = vec![];

                    let selected_ticker = match &self.selected_ticker {
                        Some(ticker) => ticker,
                        None => {
                            eprintln!("No ticker selected");
                            return Command::none();
                        }
                    };

                    let first_pane = self.first_pane;
        
                    for (pane_id, (is_open, stream_type)) in &self.panes_open {
                        if *is_open {
                            if !self.panes.panes.values().any(|pane| pane.id == *pane_id) {
                                let pane_id = *pane_id;
                                let split_pane = Command::perform(
                                    async move {
                                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                        (pane_grid::Axis::Horizontal, first_pane) 
                                    },
                                    move |(axis, pane)| {
                                        Message::Split(axis, pane, pane_id)
                                    }
                                );
                                commands.push(split_pane);
                            }
                            if *pane_id == PaneId::HeatmapChart {
                                self.heatmap_chart = Some(Heatmap::new());
                            }
                            if *pane_id == PaneId::TimeAndSales {
                                self.time_and_sales = Some(TimeAndSales::new());
                            }
                        }
                        let selected_ticker = match stream_type {
                            Some(StreamType::DepthAndTrades(ticker)) => ticker,
                            Some(StreamType::Klines(ticker, _)) => ticker,
                            _ => {
                                dbg!("No ticker selected");
                                continue; 
                            }
                        };
                        let selected_timeframe = match stream_type {
                            Some(StreamType::Klines(_, timeframe)) => timeframe,
                            _ => {
                                dbg!("No timeframe selected");
                                continue;   
                            }
                        };                            

                        let pane_id = *pane_id;
                        let fetch_klines = Command::perform(
                            market_data::fetch_klines(*selected_ticker, *selected_timeframe)
                                .map_err(|err| format!("{}", err)), 
                            move |klines| {
                                Message::FetchEvent(klines, pane_id)
                            }
                        );
                        commands.push(fetch_klines);
                    }
                    Command::batch(commands)

                } else {
                    self.ws_state = WsState::Disconnected;

                    self.heatmap_chart = None;
                    self.candlestick_chart = None;
                    self.time_and_sales = None;
                    self.custom_line = None;

                    Command::none()
                }
            },       
            Message::FetchEvent(klines, target_pane) => {
                match klines {
                    Ok(klines) => {
                        if let Some((_, Some(StreamType::Klines(_, timeframe)))) = self.panes_open.get(&target_pane) {
                            match target_pane {
                                PaneId::CustomChart => {
                                    self.custom_line = Some(CustomLine::new(klines, *timeframe));
                                },
                                PaneId::CandlestickChart => {
                                    self.candlestick_chart = Some(CustomLine::new(klines, *timeframe));
                                },
                                _ => {}
                            }
                        }
                    },
                    Err(err) => {
                        eprintln!("Error fetching klines: {}", err);
                        self.candlestick_chart = Some(CustomLine::new(vec![], Timeframe::M1)); 
                    },
                }
                Command::none()
            },
            Message::MarketWsEvent(event) => {
                match event {
                    market_data::Event::Connected(connection) => {
                        self.ws_state = WsState::Connected(connection);
                    }
                    market_data::Event::Disconnected => {
                        self.ws_state = WsState::Disconnected;
                    }
                    market_data::Event::DepthReceived(depth_update, bids, asks, trades_buffer) => {
                        if let Some(time_and_sales) = &mut self.time_and_sales {
                            time_and_sales.update(&trades_buffer);
                        } 
                        if let Some(chart) = &mut self.heatmap_chart {
                            chart.insert_datapoint(trades_buffer, depth_update, bids, asks)
                        } 
                    }
                    market_data::Event::KlineReceived(kline, timeframe) => {
                        for (pane_id, (_, stream_type_option)) in &self.panes_open {
                            if let Some(StreamType::Klines(_, pane_timeframe)) = stream_type_option {
                                if *pane_timeframe == timeframe {
                                    match pane_id {
                                        PaneId::CandlestickChart => {
                                            if let Some(candlestick_chart) = &mut self.candlestick_chart {
                                                candlestick_chart.insert_datapoint(kline.clone());
                                            }
                                        },
                                        PaneId::CustomChart => {
                                            if let Some(custom_line) = &mut self.custom_line {
                                                custom_line.insert_datapoint(kline.clone());
                                            }
                                        },
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                };
                Command::none()
            },
            Message::UserKeySucceed(listen_key) => {
                self.listen_key = Some(listen_key);
                Command::none()
            },
            Message::UserKeyError => {
                eprintln!("Check API keys");
                Command::none()
            },

            // Pane grid
            Message::Split(axis, pane, pane_id) => {
                let focus_pane = if let Some((pane, _)) = self.panes.split(axis, pane, Pane::new(pane_id)) {
                    Some(pane)
                } else if let Some((&first_pane, _)) = self.panes.panes.iter().next() {
                    self.panes.split(axis, first_pane, Pane::new(pane_id)).map(|(pane, _)| pane)
                } else {
                    None
                };

                if let Some(value) = self.panes_open.get_mut(&pane_id) {
                    value.0 = true;
                }

                if Some(focus_pane) != None {
                    self.focus = focus_pane;

                    let selected_ticker = match &self.selected_ticker {
                        Some(ticker) => ticker,
                        None => {
                            eprintln!("No ticker selected");
                            return Command::none();
                        }
                    };
                    
                    if pane_id == PaneId::TimeAndSales {
                        self.time_and_sales = Some(TimeAndSales::new());
                        self.panes_open.insert(pane_id, (true, Some(StreamType::DepthAndTrades(*selected_ticker))));
                    }
                    if pane_id == PaneId::HeatmapChart {
                        dbg!("Creating heatmap chart");
                        self.panes_open.insert(pane_id, (true, Some(StreamType::DepthAndTrades(*selected_ticker))));
                    }
                } 

                Command::none()
            },
            Message::Clicked(pane) => {
                self.focus = Some(pane);
                Command::none()
            },
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Command::none()
            },
            Message::Dragged(pane_grid::DragEvent::Dropped {
                pane,
                target,
            }) => {
                self.panes.drop(pane, target);
                Command::none()
            },
            Message::Dragged(_) => {
                Command::none()
            },
            Message::Maximize(pane) => {
                self.panes.maximize(pane);
                Command::none()
            },
            Message::Restore => {
                self.panes.restore();
                Command::none()
            },
            Message::Close(pane) => {
                self.panes.get(pane).map(|pane| {
                    match pane.id {
                        PaneId::HeatmapChart => {
                            if let Some(value) = self.panes_open.get_mut(&PaneId::HeatmapChart) {
                                value.0 = false;
                            }
                        },
                        PaneId::CandlestickChart => {
                            if let Some(value) = self.panes_open.get_mut(&PaneId::CandlestickChart) {
                                value.0 = false;
                            }
                        },
                        PaneId::CustomChart => {
                            if let Some(value) = self.panes_open.get_mut(&PaneId::CustomChart) {
                                value.0 = false;
                            }
                        },
                        PaneId::TimeAndSales => {
                            if let Some(value) = self.panes_open.get_mut(&PaneId::TimeAndSales) {
                                value.0 = false;
                            }
                        },
                        PaneId::TradePanel => {
                            if let Some(value) = self.panes_open.get_mut(&PaneId::TradePanel) {
                                value.0 = false;
                            }
                        },  
                    }
                });
                
                if let Some((_, sibling)) = self.panes.close(pane) {
                    self.focus = Some(sibling);
                }
                Command::none()
            },
            Message::ToggleLayoutLock => {
                self.focus = None;
                self.pane_lock = !self.pane_lock;
                Command::none()
            },

            Message::Debug(msg) => {
                println!("{}", msg);
                Command::none()
            },
            Message::FontLoaded(_) => {
                dbg!("Font loaded");
                Command::none()
            },

            Message::OpenModal(pane) => {
                self.panes.get_mut(pane).map(|pane| {
                    pane.show_modal = true;
                });
                Command::none()
            },
            Message::CloseModal => {
                for pane in self.panes.panes.values_mut() {
                    pane.show_modal = false;
                }
                Command::none()
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

                //self.trades_chart.as_mut().map(|chart| {
                //    chart.set_size_filter(self.size_filter_heatmap);
                //});
                self.time_and_sales.as_mut().map(|time_and_sales| {
                    time_and_sales.set_size_filter(self.size_filter_timesales);
                });

                Command::none()
            },
            Message::SyncWithHeatmap(sync) => {
                self.sync_heatmap = sync;
            
                if sync {
                    self.size_filter_heatmap = self.size_filter_timesales;
                    //self.trades_chart.as_mut().map(|chart| {
                    //    chart.set_size_filter(self.size_filter_heatmap);
                    //});
                }
            
                Command::none()
            },
            Message::CutTheKlineStream => {
                self.kline_stream = true;
                Command::none()
            },

            Message::ShowLayoutModal => {
                self.show_layout_modal = true;
                iced::widget::focus_next()
            },
            Message::HideLayoutModal => {
                self.show_layout_modal = false;
                Command::none()
            },
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
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
        
            let title = match pane.id {
                PaneId::HeatmapChart => "Heatmap Chart",
                PaneId::CandlestickChart => "Candlestick Chart",
                PaneId::CustomChart => "Custom Chart", 
                PaneId::TimeAndSales => "Time & Sales",
                PaneId::TradePanel => "Trading Panel",
            };

            if is_focused {
                let pane_timeframe = self.panes_open.get(&pane.id).and_then(|(_, stream_type)| {
                    match stream_type {
                        Some(StreamType::Klines(_, timeframe)) => Some(timeframe),
                        _ => None,
                    }
                });

                let title_bar = pane_grid::TitleBar::new(title)
                    .controls(view_controls(
                        id,
                        pane.id,
                        total_panes,
                        is_maximized,
                        pane_timeframe
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
            .push(add_pane_button)
            .push(
                tooltip(layout_lock_button, "Layout Lock", tooltip::Position::Bottom)
            );

        let ws_button = if self.selected_ticker.is_some() {
            button(if self.ws_running { "Disconnect" } else { "Connect" })
                .on_press(Message::WsToggle())
        } else {
            button(if self.ws_running { "Disconnect" } else { "Connect" })
        };
        let mut ws_controls = Row::new()
            .spacing(10)
            .align_items(Alignment::Center)
            .push(ws_button);

        if !self.ws_running {
            let symbol_pick_list = pick_list(
                &Ticker::ALL[..],
                self.selected_ticker,
                Message::TickerSelected,
            ).placeholder("Choose a ticker...");
            
            let exchange_selector = pick_list(
                &["Binance Futures"][..],
                self.selected_exchange,
                Message::ExchangeSelected,
            ).placeholder("Choose an exchange...");
        
            ws_controls = ws_controls
                .push(exchange_selector)
                .push(symbol_pick_list)
                
        } else {
            ws_controls = ws_controls.push(
                Text::new(self.selected_ticker.unwrap_or_else(|| { dbg!("No ticker found"); Ticker::BTCUSDT } ).to_string()).size(20));
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

            let candlestick_chart_button = button("Candlestick Chart")
                .width(iced::Pixels(200.0));
            if let Some((false, _)) = self.panes_open.get(&PaneId::CandlestickChart) {
                buttons = buttons.push(candlestick_chart_button.on_press(Message::Split(pane_grid::Axis::Vertical, self.first_pane, PaneId::CandlestickChart)));
            } else {
                buttons = buttons.push(candlestick_chart_button);
            }

            let custom_chart_button = button("Custom Line")
                .width(iced::Pixels(200.0));
            if let Some((false, _)) = self.panes_open.get(&PaneId::CustomChart) {
                buttons = buttons.push(custom_chart_button.on_press(Message::Split(pane_grid::Axis::Vertical, self.first_pane, PaneId::CustomChart)));
            } else {
                buttons = buttons.push(custom_chart_button);
            }

            let heatmap_chart_button = button("Heatmap Chart")
                .width(iced::Pixels(200.0));
            if let Some((false, _)) = self.panes_open.get(&PaneId::HeatmapChart) {
                buttons = buttons.push(heatmap_chart_button.on_press(Message::Split(pane_grid::Axis::Vertical, self.first_pane, PaneId::HeatmapChart)));
            } else {
                buttons = buttons.push(heatmap_chart_button);
            }

            let time_and_sales_button = button("Time & Sales")
                .width(iced::Pixels(200.0));
            if let Some((false, _)) = self.panes_open.get(&PaneId::TimeAndSales) {
                buttons = buttons.push(time_and_sales_button.on_press(Message::Split(pane_grid::Axis::Vertical, self.first_pane, PaneId::TimeAndSales)));
            } else {
                buttons = buttons.push(time_and_sales_button);
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
            self.selected_ticker.and_then(|ticker| {
                self.selected_timeframe.map(|_timeframe| {
                    let binance_market_stream = market_data::connect_market_stream(ticker).map(Message::MarketWsEvent);
                    subscriptions.push(binance_market_stream);

                    let mut streams: Vec<(Ticker, Timeframe)> = vec![];
                    
                    for (_pane_id, (_is_open, stream_type)) in &self.panes_open {
                        if let Some(StreamType::Klines(ticker, timeframe)) = stream_type {
                            streams.push((*ticker, *timeframe));
                        }
                    }
                    if !streams.is_empty() && self.kline_stream {
                        let binance_kline_streams = market_data::connect_kline_stream(streams).map(Message::MarketWsEvent);
                        subscriptions.push(binance_kline_streams);
                    }
                })
            });
        }
        
        Subscription::batch(subscriptions)
    }    

    fn theme(&self) -> Self::Theme {
        Theme::KanagawaDragon
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
                        .map(move |message| Message::Heatmap(message));
            } else {
                underlay = Text::new("No data").into();
            }

            underlay
        }, 
        
        PaneId::CandlestickChart => { 
            let underlay; 
            if let Some(candlestick_chart) = candlestick_chart {
                underlay =
                    candlestick_chart
                        .view()
                        .map(move |message| Message::Candlestick(message));
            } else {
                underlay = Text::new("No data").into();
            }
            underlay
        },

        PaneId::CustomChart => { 
            let underlay; 
            if let Some(custom_line) = custom_line {
                underlay =
                    custom_line
                        .view()
                        .map(move |message| Message::CustomLine(message));
            } else {
                underlay = Text::new("No data").into();
            }

            underlay
        },
        
        PaneId::TimeAndSales => { 
            let underlay = time_and_sales.as_ref().map(TimeAndSales::view).unwrap_or_else(|| Text::new("No data").into()); 

            underlay
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
) -> Element<'a, Message> {
    let mut row = row![].spacing(5);

    let (icon, message) = if is_maximized {
        (Icon::ResizeSmall, Message::Restore)
    } else {
        (Icon::ResizeFull, Message::Maximize(pane))
    };

    if pane_id == PaneId::CandlestickChart || pane_id == PaneId::CustomChart {
        let timeframe_picker = pick_list(
            &Timeframe::ALL[..],
            selected_timeframe,
            move |timeframe| Message::TimeframeSelected(timeframe, pane),
        ).placeholder("Choose a timeframe...").text_size(11).width(iced::Pixels(80.0));
        row = row.push(timeframe_picker);
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
            let trade_time = NaiveDateTime::from_timestamp(trade.time as i64 / 1000, (trade.time % 1000) as u32 * 1_000_000);
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
            trades_column = trades_column.push(Text::new("No trades").size(16));
        } else {
            for trade in filtered_trades.iter().rev().take(80) {
                let trade: &ConvertedTrade = *trade;

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
                            .width(Length::FillPortion(4))
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

    pub fn title_bar_active(theme: &Theme) -> Style {
        let palette = theme.extended_palette();

        Style {
            text_color: Some(palette.background.strong.text),
            background: Some(palette.background.strong.color.into()),
            border: Border {
                width: 1.0,
                color: palette.primary.strong.color,
                radius: 4.0.into(), 
                ..Border::default()
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
                color: Color::from_rgba(192.0 / 255.0, 80.0 / 255.0, 77.0 / 255.0, color_alpha).into(),
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
                color: Color::from_rgba(81.0 / 255.0, 205.0 / 255.0, 160.0 / 255.0, color_alpha).into(),
                ..Border::default()
            },
            ..Default::default()
        }
    }
}