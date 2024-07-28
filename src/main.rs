#![windows_subsystem = "windows"]

mod data_providers;
mod charts;
mod style;
mod screen;

use style::{ICON_FONT, ICON_BYTES, Icon};
use screen::dashboard::{Dashboard, PaneContent, PaneSettings, PaneState, Uuid};
use data_providers::{binance, bybit, BinanceWsState, BybitWsState, UserWsState, Exchange, MarketEvents, TickMultiplier, Ticker, Timeframe, StreamType};

use charts::footprint::FootprintChart;
use charts::heatmap::HeatmapChart;
use charts::candlestick::CandlestickChart;
use charts::timeandsales::TimeAndSales;

use futures::TryFutureExt;

use std::{collections::{HashMap, HashSet, VecDeque}, vec};

use iced::{
    alignment, font, widget::{
        button, center, checkbox, mouse_area, opaque, pick_list, stack, tooltip, Column, Container, Row, Slider, Space, Text
    }, Alignment, Color, Task, Element, Font, Length, Renderer, Settings, Size, Subscription, Theme
};
use iced::widget::pane_grid::{self, PaneGrid, Configuration};
use iced::widget::{
    container, row, scrollable, text, responsive
};

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
pub enum Message {
    Debug(String),
    ErrorOccurred(String),

    ChartUserUpdate(charts::Message, Uuid),
    ShowLayoutModal,
    HideLayoutModal,

    // Market&User data stream
    UserKeySucceed(String),
    UserKeyError,
    TickerSelected(Ticker, Uuid),
    TimeframeSelected(Timeframe, pane_grid::Pane),
    ExchangeSelected(Exchange, Uuid),
    MarketWsEvent(MarketEvents),
    WsToggle,
    FetchEvent(Result<Vec<data_providers::Kline>, std::string::String>, StreamType),
    RestartStream(Option<pane_grid::Pane>, (Option<Ticker>, Option<Timeframe>, Option<f32>)),
    CutTheKlineStream,
    
    // Pane grid
    Split(pane_grid::Axis, pane_grid::Pane, Uuid),
    Clicked(pane_grid::Pane),
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    Maximize(pane_grid::Pane),
    Restore,
    Close(pane_grid::Pane),
    ToggleLayoutLock,
    PaneContentSelected(String, Uuid, Vec<StreamType>),

    // Modal
    OpenModal(pane_grid::Pane),
    CloseModal(Uuid),

    // Slider
    SliderChanged(PaneId, f32),
    SyncWithHeatmap(bool),

    // Chart settings
    TicksizeSelected(TickMultiplier, Uuid),
    SetMinTickSize(f32, Uuid),   
}

struct State {
    dashboard: Dashboard,

    exchange_latency: Option<(u32, u32)>,

    listen_key: Option<String>,

    binance_ws_state: BinanceWsState,
    bybit_ws_state: BybitWsState,
    user_ws_state: UserWsState,

    kline_stream: bool,
    ws_running: bool,

    feed_latency_cache: VecDeque<data_providers::FeedLatency>,

    pane_streams: HashMap<Exchange, HashMap<Ticker, HashSet<StreamType>>>,
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
                            settings: PaneSettings::default(),
                        })
                    ),
                    b: Box::new(Configuration::Pane(
                        PaneState { 
                            id: Uuid::new_v4(), 
                            show_modal: false, 
                            stream: vec![],
                            content: PaneContent::Starter,
                            settings: PaneSettings::default(),
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
                            settings: PaneSettings::default(),
                        })                      
                    ),
                    b: Box::new(Configuration::Pane(
                        PaneState { 
                            id: Uuid::new_v4(), 
                            show_modal: false, 
                            stream: vec![],
                            content: PaneContent::Starter,
                            settings: PaneSettings::default(),
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
                    settings: PaneSettings::default(),
                })
            ),
        };
        let dashboard = Dashboard::empty(pane_config);
        
        Self { 
            dashboard,
            kline_stream: true,
            listen_key: None,
            binance_ws_state: BinanceWsState::Disconnected,
            bybit_ws_state: BybitWsState::Disconnected,
            user_ws_state: UserWsState::Disconnected,
            ws_running: false,
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
                        
                        dbg!("Min tick size set");
                    },
                    Err(err) => {
                        dbg!("Failed to set min tick size: {}", err);
                    }
                }

                Task::none()
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
                match self.dashboard.pane_change_ticksize(pane_id, tick_multiply) {
                    Ok(_) => {
                        dbg!("Ticksize changed");
                    },
                    Err(err) => {
                        dbg!("Failed to change ticksize:, {}", err);
                    }
                };

                Task::none()
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
            Message::FetchEvent(klines, pane_stream) => {
                match klines {
                    Ok(klines) => {
                        match pane_stream {
                            StreamType::Kline { exchange, ticker, timeframe } => {
                                self.dashboard.insert_klines_vec(&StreamType::Kline {
                                    exchange,
                                    ticker,
                                    timeframe,
                                }, &klines);
                            },
                            _ => {}
                        }
                    },
                    Err(err) => {
                        eprintln!("{err}");
                    }
                }

                Task::perform(
                    async {
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    },
                    move |()| Message::CutTheKlineStream
                )
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
                            let stream_type = StreamType::DepthAndTrades {
                                exchange: Exchange::BinanceFutures,
                                ticker,
                            };
                            
                            self.dashboard.update_depth_and_trades(stream_type, depth_update_t, depth, trades_buffer);
                        }
                        binance::market_data::Event::KlineReceived(ticker, kline, timeframe) => {
                            let stream_type = StreamType::Kline {
                                exchange: Exchange::BinanceFutures,
                                ticker,
                                timeframe,
                            };

                            self.dashboard.update_latest_klines(&stream_type, &kline);
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
                            let stream_type = StreamType::DepthAndTrades {
                                exchange: Exchange::BybitLinear,
                                ticker,
                            };
                            
                            self.dashboard.update_depth_and_trades(stream_type, depth_update_t, depth, trades_buffer);
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
                let mut tasks = vec![];
                
                let pane_content = match content.as_str() {
                    "Heatmap chart" => PaneContent::Heatmap(HeatmapChart::new()),
                    "Footprint chart" => {
                        let footprint_chart = FootprintChart::new(1, 1.0, vec![], vec![]);
                        PaneContent::Footprint(footprint_chart)
                    },
                    "Candlestick chart" => {
                        let candlestick_chart = CandlestickChart::new(vec![], Timeframe::M1);
                        PaneContent::Candlestick(candlestick_chart)
                    },
                    "Time&Sales" => PaneContent::TimeAndSales(TimeAndSales::new()),
                    _ => return Task::none(),
                };
                
                if let Ok(vec_streams) = self.dashboard.get_pane_stream_mut(pane_id) {
                    *vec_streams = pane_stream.to_vec();
                } else {
                    dbg!("No pane found for stream update");
                }
            
                if let Err(err) = self.dashboard.set_pane_content(pane_id, pane_content) {
                    dbg!("Failed to set pane content: {}", err);
                } else {
                    dbg!("Pane content set");
                }
            
                if content == "Footprint chart" || content == "Candlestick chart" {
                    self.kline_stream = false;
                    for stream in pane_stream.iter() {
                        if let StreamType::Kline { ticker, timeframe, .. } = stream {
                            let stream_clone = stream.clone();

                            let fetch_klines = Task::perform(
                                binance::market_data::fetch_klines(*ticker, *timeframe)
                                    .map_err(|err| format!("{err}")),
                                move |klines| Message::FetchEvent(klines, stream_clone)
                            );

                            tasks.push(fetch_klines);
                            
                            if content == "Footprint chart" {
                                let fetch_ticksize: Task<Message> = Task::perform(
                                    binance::market_data::fetch_ticksize(*ticker),
                                    move |result| match result {
                                        Ok(ticksize) => Message::SetMinTickSize(ticksize, pane_id),
                                        Err(err) => {
                                            Message::ErrorOccurred(err.to_string())
                                        }
                                    }
                                );

                                tasks.push(fetch_ticksize);
                            }    
                        }
                    }
                }
            
                for stream in pane_stream.iter() {
                    match stream {
                        StreamType::Kline { exchange, ticker, .. } | StreamType::DepthAndTrades { exchange, ticker } => {
                            self.pane_streams
                                .entry(*exchange)
                                .or_insert_with(HashMap::new)
                                .entry(*ticker)
                                .or_insert_with(HashSet::new)
                                .insert(stream.clone());
                        }
                        _ => {}
                    }
                }
            
                dbg!(&self.pane_streams);
            
                Task::batch(tasks)
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
                    StreamType::Kline { exchange, ticker, timeframe } => {
                        format!("{} {} {}", exchange, ticker, timeframe)
                    }
                    StreamType::DepthAndTrades { exchange, ticker } => {
                        format!("{} {}", exchange, ticker)
                    }
                    _ => "Err stream type".to_string()
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
                    text(char::from(Icon::Locked).to_string()).font(ICON_FONT) 
                } else { 
                    text(char::from(Icon::Unlocked).to_string()).font(ICON_FONT) 
                })
                .width(25)
                .center_x(iced::Pixels(20.0))
            )
            .on_press(Message::ToggleLayoutLock);

        let add_pane_button = button(
            container(
                text(char::from(Icon::Layout).to_string()).font(ICON_FONT))
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

        let debug_button = button("Debug").on_press(Message::Debug("Debug".to_string()));

        let mut ws_controls = Row::new()
            .spacing(10)
            .align_items(Alignment::Center)
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
                            Text::new(format!("{} ms", highest_latency)).size(10)
                        )
                );
            
            ws_controls = ws_controls.push(
                Row::new()
                    .spacing(10)
                    .align_items(Alignment::Center)
                    .push(tooltip(exchange_info, exchange_latency_tooltip, tooltip::Position::Bottom).style(style::tooltip))
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
                    let mut depth_streams = Vec::new();

                    let mut kline_streams = Vec::new();

                    for (_, stream_types) in stream {
                        for stream_type in stream_types {
                            match stream_type {
                                StreamType::Kline { exchange, ticker, timeframe } => {
                                    kline_streams.push((ticker, timeframe));
                                },
                                StreamType::DepthAndTrades { exchange, ticker } => {
                                    depth_streams.push(
                                        binance::market_data::connect_market_stream(*ticker)
                                            .map(|event: binance::market_data::Event| Message::MarketWsEvent(MarketEvents::Binance(event)))
                                    );
                                },
                                _ => {}
                            }
                        }
                    }

                    if kline_streams.len() > 0 && self.kline_stream {
                        let mut streams: Vec<(Ticker, Timeframe)> = vec![];
                            
                        for (ticker, timeframe) in kline_streams {
                            streams.push((*ticker, *timeframe));
                        }

                        subscriptions.push(
                            binance::market_data::connect_kline_stream(streams)
                                .map(move |event: binance::market_data::Event| Message::MarketWsEvent(MarketEvents::Binance(event)))
                        );
                    }

                    subscriptions.push(Subscription::batch(depth_streams));
                },
                Exchange::BybitLinear => {
                    let mut bybit_streams = Vec::new();

                    for (_, stream_types) in stream {
                        for stream_type in stream_types {
                            match stream_type {
                                StreamType::Kline { exchange, ticker, timeframe } => {
                                },
                                StreamType::DepthAndTrades { exchange, ticker } => {
                                    bybit_streams.push(
                                        bybit::market_data::connect_market_stream(*ticker)
                                            .map(|event: bybit::market_data::Event| Message::MarketWsEvent(MarketEvents::Bybit(event)))
                                    );
                                },
                                _ => {}
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
                if let (Some(exchange), Some(ticker)) = (pane.settings.selected_exchange, pane.settings.selected_ticker) {
                    let timeframe = pane.settings.selected_timeframe.unwrap_or_else(|| { dbg!("No timeframe found"); Timeframe::M1 });

                    let pane_stream: Vec<StreamType> = match label {
                        "Heatmap chart" => vec![StreamType::DepthAndTrades { exchange, ticker }],
                        "Footprint chart" => vec![StreamType::DepthAndTrades { exchange, ticker }, StreamType::Kline { exchange, ticker, timeframe }],
                        "Candlestick chart" => vec![StreamType::Kline { exchange, ticker, timeframe }],
                        "Time&Sales" => vec![StreamType::DepthAndTrades { exchange, ticker }],
                        _ => vec![]
                    };
                
                    btn = btn.on_press(
                        Message::PaneContentSelected(label.to_string(), pane.id, pane_stream)
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
        (container(text(char::from(Icon::Cog).to_string()).font(ICON_FONT).size(14)).width(25).center_x(iced::Pixels(25.0)), Message::OpenModal(pane)),
        (container(text(char::from(icon).to_string()).font(ICON_FONT).size(14)).width(25).center_x(iced::Pixels(25.0)), message),
    ];

    if total_panes > 1 {
        buttons.push((container(text(char::from(Icon::Close).to_string()).font(ICON_FONT).size(14)).width(25).center_x(iced::Pixels(25.0)), Message::Close(pane)));
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