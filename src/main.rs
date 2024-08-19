#![windows_subsystem = "windows"]

mod data_providers;
mod charts;
mod style;
mod screen;
mod logger;

use hyper::client::conn;
use style::{ICON_FONT, ICON_BYTES, Icon};

use screen::{dashboard, Error, Notification};
use screen::dashboard::{
    Dashboard,
    pane::{self, SerializablePane}, Uuid, LayoutId,
    PaneContent, PaneSettings, PaneState, 
    SavedState, SerializableDashboard, SerializableState, 
    read_layout_from_file, write_json_to_file, 
};
use data_providers::{binance, bybit, Exchange, MarketEvents, TickMultiplier, Ticker, Timeframe, StreamType};

use charts::footprint::FootprintChart;
use charts::heatmap::HeatmapChart;
use charts::candlestick::CandlestickChart;
use charts::timeandsales::TimeAndSales;

use futures::TryFutureExt;

use std::{collections::{HashMap, HashSet, VecDeque}, vec};

use iced::{
    alignment, widget::{
        button, center, checkbox, mouse_area, opaque, pick_list, stack, tooltip, Column, Container, Row, Slider, Space, Text
    }, window::{self, Position}, Alignment, Color, Element, Length, Point, Renderer, Size, Subscription, Task, Theme
};
use iced::widget::pane_grid::{self, PaneGrid, Configuration};
use iced::widget::{container, row, scrollable, text};

fn main() -> iced::Result {
    logger::setup(false, false).expect("Failed to initialize logger");

    let saved_state = match read_layout_from_file("dashboard_state.json") {
        Ok(state) => {
            let mut de_state = SavedState {
                layouts: HashMap::new(),
                last_active_layout: state.last_active_layout,
                window_size: state.window_size,
                window_position: state.window_position,
            };

            fn configuration(pane: SerializablePane) -> Configuration<PaneState> {
                match pane {
                    SerializablePane::Split { axis, ratio, a, b } => Configuration::Split {
                        axis: match axis {
                            pane::Axis::Horizontal => pane_grid::Axis::Horizontal,
                            pane::Axis::Vertical => pane_grid::Axis::Vertical,
                        },
                        ratio,
                        a: Box::new(configuration(*a)),
                        b: Box::new(configuration(*b)),
                    },
                    SerializablePane::Starter => {
                        Configuration::Pane(PaneState::new(Uuid::new_v4(), vec![], PaneSettings::default()))
                    },
                    SerializablePane::CandlestickChart { stream_type, settings } => {
                        let timeframe = settings.selected_timeframe
                            .unwrap()
                            .to_minutes();

                        Configuration::Pane(
                            PaneState::from_config(
                                PaneContent::Candlestick(
                                    CandlestickChart::new(
                                        vec![], 
                                        timeframe
                                    )
                                ),
                                stream_type,
                                settings
                            )
                        )
                    },
                    SerializablePane::FootprintChart { stream_type, settings } => {
                        let ticksize = settings.tick_multiply
                            .unwrap()
                            .multiply_with_min_tick_size(
                                settings.min_tick_size
                                    .expect("No min tick size found, deleting dashboard_state.json probably fixes this")
                            );
                    
                        let timeframe = settings.selected_timeframe
                            .unwrap()
                            .to_minutes();

                        Configuration::Pane(
                            PaneState::from_config(
                                PaneContent::Footprint(
                                    FootprintChart::new(
                                        timeframe,
                                        ticksize,
                                        vec![], 
                                        vec![]
                                    )
                                ),
                                stream_type,
                                settings
                            )
                        )
                    },
                    SerializablePane::HeatmapChart { stream_type, settings } => {
                        let ticksize = settings.tick_multiply
                            .unwrap()
                            .multiply_with_min_tick_size(
                                settings.min_tick_size
                                    .expect("No min tick size found, deleting dashboard_state.json probably fixes this")
                            );

                        Configuration::Pane(
                            PaneState::from_config(
                                PaneContent::Heatmap(
                                    HeatmapChart::new(ticksize)
                                ),
                                stream_type,
                                settings
                            )
                        )
                    },
                    SerializablePane::TimeAndSales { stream_type, settings } => {
                        Configuration::Pane(
                            PaneState::from_config(
                                PaneContent::TimeAndSales(
                                    TimeAndSales::new()
                                ),
                                stream_type,
                                settings
                            )
                        )
                    },
                }
            }

            for (id, dashboard) in state.layouts.iter() {                
                let dashboard = Dashboard::from_config(configuration(dashboard.pane.clone()));

                de_state.layouts.insert(*id, dashboard);
            }

            de_state
        },
        Err(e) => {
            log::error!("Failed to load/find layout state: {}. Starting with a new layout.", e);

            SavedState::default()
        }
    };

    let window_size = saved_state.window_size.unwrap_or((1600.0, 900.0));
    let window_position = saved_state.window_position.unwrap_or((0.0, 0.0));

    let window_settings = window::Settings {
        size: iced::Size::new(window_size.0, window_size.1),
        position: Position::Specific(Point::new(window_position.0, window_position.1)),
        ..Default::default()
    };

    iced::application(
        "Iced Trade",
        State::update,
        State::view,
    )
    .subscription(State::subscription)
    .theme(|_| Theme::KanagawaDragon)
    .antialiasing(true)
    .window(window_settings)
    .centered()   
    .font(ICON_BYTES)
    .exit_on_close_request(false)
    .run_with(move || State::new(saved_state))
}

#[derive(Debug, Clone)]
pub enum Message {
    FetchDistributeKlines(StreamType, Result<Vec<data_providers::Kline>, std::string::String>),
    FetchDistributeTicks(StreamType, Result<f32, std::string::String>),
    Debug(String),
    Notification(Notification),
    ErrorOccurred(Error),
    ClearNotification,

    ChartUserUpdate(charts::Message, Uuid),
    ShowLayoutModal,
    HideLayoutModal,
    ShowPanesModal,
    HidePanesModal,

    // Market&User data stream
    UserKeySucceed(String),
    UserKeyError,
    TickerSelected(Ticker, Uuid),
    ExchangeSelected(Exchange, Uuid),
    MarketWsEvent(MarketEvents),
    FetchEvent(Result<Vec<data_providers::Kline>, std::string::String>, StreamType, Uuid),
    
    // Pane grid
    ToggleLayoutLock,
    PaneContentSelected(String, Uuid, Vec<StreamType>),
    ReplacePane(pane_grid::Pane),

    // Modal
    OpenModal(pane_grid::Pane),
    CloseModal(Uuid),

    // Slider
    SliderChanged(Uuid, f32),
    SyncWithHeatmap(bool),

    // Chart settings
    TicksizeSelected(TickMultiplier, Uuid),
    TimeframeSelected(Timeframe, Uuid),
    SetMinTickSize(f32, Uuid),   
    
    Event(Event),

    SaveAndExit(window::Id, Option<Size>, Option<Point>),

    ResetCurrentLayout,
    LayoutSelected(LayoutId),
    Dashboard(dashboard::Message),
}

struct State {
    layouts: HashMap<LayoutId, Dashboard>,
    last_active_layout: LayoutId,
    show_layout_modal: bool,
    exchange_latency: Option<(u32, u32)>,
    listen_key: Option<String>,
    feed_latency_cache: VecDeque<data_providers::FeedLatency>,
    pane_streams: HashMap<Exchange, HashMap<Ticker, HashSet<StreamType>>>,
    notification: Option<Notification>,
}

impl State {
    fn new(saved_state: SavedState) -> (Self, Task<Message>) {
        let mut tasks = vec![];

        let mut pane_streams = HashMap::new();

        let last_active_layout = saved_state.last_active_layout;
        let dashboard = saved_state.layouts.get(&last_active_layout);

        if let Some(dashboard) = dashboard {
            let sleep_and_fetch = Task::perform(
                async { tokio::time::sleep(tokio::time::Duration::from_millis(200)).await; },
                move |_| Message::LayoutSelected(last_active_layout)
            );

            tasks.push(sleep_and_fetch);

            pane_streams = dashboard.get_all_diff_streams();
        }

        (
            Self { 
                layouts: saved_state.layouts,
                last_active_layout,
                show_layout_modal: false,
                listen_key: None,
                exchange_latency: None,
                feed_latency_cache: VecDeque::new(),
                pane_streams,
                notification: None,
            },
            Task::batch(tasks)
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ChartUserUpdate(message, pane_id) => {
                let dashboard = self.get_mut_dashboard();
            
                match dashboard.update_chart_state(pane_id, message) {
                    Ok(_) => Task::none(),
                    Err(err) => {      
                        Task::perform(
                            async { err },
                            move |err: Error| Message::ErrorOccurred(err)
                        )
                    }
                }
            },
            Message::SetMinTickSize(min_tick_size, pane_id) => {
                let dashboard = self.get_mut_dashboard();

                match dashboard.get_pane_settings_mut(pane_id) {
                    Ok(pane_settings) => {
                        pane_settings.min_tick_size = Some(min_tick_size);
                        
                        Task::none()
                    },
                    Err(err) => {
                        Task::perform(
                            async { err },
                            move |err: Error| Message::ErrorOccurred(err)
                        )
                    }
                }
            },
            Message::TickerSelected(ticker, pane_id) => {
                let dashboard = self.get_mut_dashboard();

                match dashboard.get_pane_settings_mut(pane_id) {
                    Ok(pane_settings) => {
                        pane_settings.selected_ticker = Some(ticker);
                        
                        Task::none()
                    },
                    Err(err) => {
                        Task::perform(
                            async { err },
                            move |err: Error| Message::ErrorOccurred(err)
                        )
                    }
                }
            },
            Message::TimeframeSelected(timeframe, pane_id) => {    
                let mut tasks = vec![];

                let dashboard = self.get_mut_dashboard();
            
                match dashboard.set_pane_timeframe(pane_id, timeframe) {
                    Ok(stream_type) => {
                        if let StreamType::Kline { exchange, ticker, timeframe } = stream_type {
                            let stream = *stream_type;
            
                            match exchange {
                                Exchange::BinanceFutures => {
                                    tasks.push(
                                        Task::perform(
                                            binance::market_data::fetch_klines(*ticker, *timeframe)
                                                .map_err(|err| format!("{err}")),
                                            move |klines| Message::FetchEvent(klines, stream, pane_id)
                                        )
                                    );
                                },
                                Exchange::BybitLinear => {                                    
                                    tasks.push(
                                        Task::perform(
                                            bybit::market_data::fetch_klines(*ticker, *timeframe)
                                                .map_err(|err| format!("{err}")),
                                            move |klines| Message::FetchEvent(klines, stream, pane_id)
                                        )
                                    );
                                },
                            }

                            tasks.push(
                                Task::perform(
                                    async {},
                                    move |_| Message::Notification(Notification::Info("Fetching for klines...".to_string()))
                                )
                            );

                            self.pane_streams = dashboard.get_all_diff_streams();
                        }
                    },
                    Err(err) => {
                        tasks.push(Task::perform(
                            async { err },
                            move |err: Error| Message::ErrorOccurred(err)
                        ));
                    }
                }

                Task::batch(tasks)
            },
            Message::ExchangeSelected(exchange, pane_id) => {
                let dashboard = self.get_mut_dashboard();

                match dashboard.get_pane_settings_mut(pane_id) {
                    Ok(pane_settings) => {
                        pane_settings.selected_exchange = Some(exchange);

                        Task::none()
                    },
                    Err(err) => {
                        Task::perform(
                            async { err },
                            move |err: Error| Message::ErrorOccurred(err)
                        )
                    }
                }
            },
            Message::TicksizeSelected(tick_multiply, pane_id) => {
                let dashboard = self.get_mut_dashboard();
                
                match dashboard.set_pane_ticksize(pane_id, tick_multiply) {
                    Ok(_) => {
                        Task::none()
                    },
                    Err(err) => {            
                        Task::perform(
                            async { err },
                            move |err: Error| Message::ErrorOccurred(err)
                        )
                    }
                }
            },
            Message::FetchEvent(klines, pane_stream, pane_id) => {
                if let Some(notification) = &self.notification {
                    match notification {
                        Notification::Info(_) => {
                            self.notification = None;
                        },
                        _ => {}
                    }
                }
               
                let dashboard = self.get_mut_dashboard();

                match klines {
                    Ok(klines) => {
                        if let StreamType::Kline { .. } = pane_stream {
                            dashboard.insert_klines_vec(&pane_stream, &klines, pane_id);

                            Task::none()
                        } else {
                            log::error!("Invalid stream type for klines: {pane_stream:?}");

                            Task::none()
                        }
                    },
                    Err(err) => {
                        Task::perform(
                            async { err },
                            move |err: String| Message::ErrorOccurred(Error::FetchError(err))
                        )
                    }
                }
            },
            Message::MarketWsEvent(event) => {
                let dashboard = self.get_mut_dashboard();

                match event {
                    MarketEvents::Binance(event) => match event {
                        binance::market_data::Event::Connected(connection) => {
                            log::info!("a stream connected to Binance WS: {connection:?}");
                        }
                        binance::market_data::Event::Disconnected(event) => {
                            log::info!("a stream disconnected from Binance WS: {event:?}");
                        }
                        binance::market_data::Event::DepthReceived(ticker, feed_latency, depth_update_t, depth, trades_buffer) => {                            
                            let stream_type = StreamType::DepthAndTrades {
                                exchange: Exchange::BinanceFutures,
                                ticker,
                            };
                            
                            if let Err(err) = dashboard.update_depth_and_trades(stream_type, depth_update_t, depth, trades_buffer) {
                                log::error!("{err}, {stream_type:?}");

                                self.pane_streams
                                    .entry(Exchange::BinanceFutures)
                                    .or_default()
                                    .entry(ticker)
                                    .or_default()
                                    .remove(&stream_type);
                            }
                        }
                        binance::market_data::Event::KlineReceived(ticker, kline, timeframe) => {
                            let stream_type = StreamType::Kline {
                                exchange: Exchange::BinanceFutures,
                                ticker,
                                timeframe,
                            };

                            if let Err(err) = dashboard.update_latest_klines(&stream_type, &kline) {
                                log::error!("{err}, {stream_type:?}");

                                self.pane_streams
                                    .entry(Exchange::BinanceFutures)
                                    .or_default()
                                    .entry(ticker)
                                    .or_default()
                                    .remove(&stream_type);
                            }
                        }
                    },
                    MarketEvents::Bybit(event) => match event {
                        bybit::market_data::Event::Connected(_) => {
                            log::info!("a stream connected to Bybit WS");
                        }
                        bybit::market_data::Event::Disconnected(event) => {
                            log::info!("a stream disconnected from Bybit WS: {event:?}");
                        }
                        bybit::market_data::Event::DepthReceived(ticker, feed_latency, depth_update_t, depth, trades_buffer) => {
                            let stream_type = StreamType::DepthAndTrades {
                                exchange: Exchange::BybitLinear,
                                ticker,
                            };
                            
                            if let Err(err) = dashboard.update_depth_and_trades(stream_type, depth_update_t, depth, trades_buffer) {
                                log::error!("{err}, {stream_type:?}");

                                self.pane_streams
                                    .entry(Exchange::BybitLinear)
                                    .or_default()
                                    .entry(ticker)
                                    .or_default()
                                    .remove(&stream_type);
                            }
                        }
                        bybit::market_data::Event::KlineReceived(ticker, kline, timeframe) => {
                            let stream_type = StreamType::Kline {
                                exchange: Exchange::BybitLinear,
                                ticker,
                                timeframe,
                            };

                            if let Err(err) = dashboard.update_latest_klines(&stream_type, &kline) {
                                log::error!("{err}, {stream_type:?}");

                                self.pane_streams
                                    .entry(Exchange::BybitLinear)
                                    .or_default()
                                    .entry(ticker)
                                    .or_default()
                                    .remove(&stream_type);
                            }
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
                log::error!("Check API keys");
                Task::none()
            },
            Message::ToggleLayoutLock => {
                let dashboard = self.get_mut_dashboard();

                dashboard.layout_lock = !dashboard.layout_lock;

                dashboard.focus = None;

                Task::none()
            },
            Message::Debug(msg) => {
                println!("{msg}");
                
                Task::none()
            },
            Message::Event(event) => {
                if let Event::CloseRequested(window) = event {     
                    enum Either<L, R> {
                        Left(L),
                        Right(R),
                    }

                    Task::batch(vec![
                        window::get_size(window).map(Either::Left),
                        window::get_position(window).map(Either::Right)
                    ])
                    .collect()
                    .map(move |results| {
                        let mut size = None;
                        let mut position = None;
                        for result in results {
                            match result {
                                Either::Left(s) => size = Some(s),
                                Either::Right(p) => position = p,
                            }
                        }
                        Message::SaveAndExit(window, size, position)
                    })
                } else {
                    Task::none()
                }
            },
            Message::SaveAndExit(window, size, position) => {
                let mut layouts = HashMap::new();

                for (id, dashboard) in self.layouts.iter() {
                    let serialized_dashboard = SerializableDashboard::from(dashboard);

                    layouts.insert(*id, serialized_dashboard);
                }

                let layout = SerializableState::from_parts(
                    layouts,
                    self.last_active_layout,
                    size,
                    position
                );
            
                match serde_json::to_string(&layout) {
                    Ok(layout_str) => {
                        if let Err(e) = write_json_to_file(&layout_str, "dashboard_state.json") {
                            log::error!("Failed to write layout state to file: {}", e);
                        } else {
                            log::info!("Successfully wrote layout state to dashboard_state.json");
                        }
                    },
                    Err(e) => log::error!("Failed to serialize layout: {}", e),
                }
            
                window::close(window)
            },
            Message::OpenModal(pane) => {
                let dashboard = self.get_mut_dashboard();

                if let Some(pane) = dashboard.panes.get_mut(pane) {
                    pane.show_modal = true;
                };
                Task::none()
            },
            Message::CloseModal(pane_id) => {
                let dashboard = self.get_mut_dashboard();
                
                for (_, pane_state) in dashboard.panes.iter_mut() {
                    if pane_state.id == pane_id {
                        pane_state.show_modal = false;
                    }
                }
                Task::none()
            },
            Message::SliderChanged(pane_id, value) => {
                let dashboard = self.get_mut_dashboard();

                match dashboard.set_pane_size_filter(pane_id, value) {
                    Ok(_) => {
                        log::info!("Size filter set to {value}");

                        Task::none()
                    }
                    Err(err) => {
                        Task::perform(
                            async { err },
                            move |err: Error| Message::ErrorOccurred(err)
                        )
                    }
                }
            },
            Message::SyncWithHeatmap(sync) => {   
                Task::perform(
                    async {},
                    move |_| Message::Notification(
                        Notification::Warn("gonna have to reimplement that".to_string())
                    )
                )
            },
            Message::ShowLayoutModal => {
                let dashboard = self.get_mut_dashboard();

                if dashboard.show_panes_modal {
                    dashboard.show_panes_modal = false;
                }

                self.show_layout_modal = true;
                iced::widget::focus_next()
            },
            Message::HideLayoutModal => {
                self.show_layout_modal = false;
                Task::none()
            },
            Message::ShowPanesModal => {
                if self.show_layout_modal {
                    self.show_layout_modal = false;
                }

                let dashboard = self.get_mut_dashboard();

                dashboard.show_panes_modal = true;
                iced::widget::focus_next()
            },
            Message::HidePanesModal => {
                let dashboard = self.get_mut_dashboard();

                dashboard.show_panes_modal = false;
                Task::none()
            },
            Message::Notification(notification) => {
                self.notification = Some(notification);

                Task::perform(
                    async { tokio::time::sleep(tokio::time::Duration::from_millis(4000)).await },
                    move |_| Message::ClearNotification
                )
            },
            Message::ErrorOccurred(err) => {
                match err {
                    Error::FetchError(err) => {
                        log::error!("{err}");

                        Task::perform(
                            async {},
                            move |_| Message::Notification(
                                Notification::Error(format!("Failed to fetch data: {err}"))
                            )
                        )
                    },
                    Error::PaneSetError(err) => {
                        log::error!("{err}");

                        Task::perform(
                            async {},
                            move |_| Message::Notification(
                                Notification::Error(format!("Failed to set pane: {err}"))
                            )
                        )
                    },
                    Error::ParseError(err) => {
                        log::error!("{err}");

                        Task::perform(
                            async {},
                            move |_| Message::Notification(
                                Notification::Error(format!("Failed to parse data: {err}"))
                            )
                        )
                    },
                    Error::StreamError(err) => {
                        log::error!("{err}");

                        Task::perform(
                            async {},
                            move |_| Message::Notification(
                                Notification::Error(format!("Failed to fetch stream: {err}"))
                            )
                        )
                    },
                    Error::UnknownError(err) => {
                        log::error!("{err}");

                        Task::perform(
                            async {},
                            move |_| Message::Notification(
                                Notification::Error(format!("{err}"))
                            )
                        )
                    },
                }
            },
            Message::ClearNotification => {
                self.notification = None;

                Task::none()
            },
            Message::PaneContentSelected(content, pane_id, pane_stream) => {
                let dashboard = self.get_mut_dashboard();

                let mut tasks = vec![];
                    
                let pane_content = match content.as_str() {
                    "Heatmap chart" => PaneContent::Heatmap(
                        HeatmapChart::new(1.0)
                    ),
                    "Footprint chart" => {
                        PaneContent::Footprint(
                            FootprintChart::new(1, 1.0, vec![], vec![])
                        )
                    },
                    "Candlestick chart" => {
                        PaneContent::Candlestick(
                            CandlestickChart::new(vec![], 1)
                        )
                    },
                    "Time&Sales" => PaneContent::TimeAndSales(
                        TimeAndSales::new()
                    ),
                    _ => return Task::none(),
                };

                // set pane's stream and content identifiers
                if let Err(err) = dashboard.set_pane_content(pane_id, pane_content) {
                    log::error!("Failed to set pane content: {}", err);
                } else {
                    log::info!("Pane content set: {content}");
                }
                
                if let Err(err) = dashboard.set_pane_stream(pane_id, pane_stream.to_vec()) {
                    log::error!("Failed to set pane stream: {err}");
                } else {
                    log::info!("Pane stream set: {pane_stream:?}");
                }
            
                // prepare unique streams for websocket
                for stream in pane_stream.iter() {
                    match stream {
                        StreamType::Kline { exchange, ticker, .. } | StreamType::DepthAndTrades { exchange, ticker } => {
                            self.pane_streams
                                .entry(*exchange)
                                .or_default()
                                .entry(*ticker)
                                .or_default()
                                .insert(*stream);
                        }
                        _ => {}
                    }
                }
            
                log::info!("{:?}", &self.pane_streams);

                // get fetch tasks for pane's content
                if ["Footprint chart", "Candlestick chart", "Heatmap chart"].contains(&content.as_str()) {
                    for stream in pane_stream.iter() {
                        match stream {
                            StreamType::Kline { exchange, ticker, .. } => {
                                if ["Candlestick chart", "Footprint chart"].contains(&content.as_str()) {
                                    tasks.push(create_fetch_klines_task(*stream, pane_id));
                                    
                                    if content == "Footprint chart" {
                                        tasks.push(create_fetch_ticksize_task(exchange, ticker, pane_id));
                                    }
                                }
                            },
                            StreamType::DepthAndTrades { exchange, ticker } => {
                                tasks.push(create_fetch_ticksize_task(exchange, ticker, pane_id));
                            },
                            _ => {}
                        }
                    }

                    tasks.push(
                        Task::perform(
                            async {},
                            move |_| Message::Notification(
                                Notification::Info(format!("Fetching data for the {}...", content.to_lowercase()))
                            )
                        )
                    );
                }
                
                Task::batch(tasks)
            },
            Message::ReplacePane(pane) => {
                let dashboard = self.get_mut_dashboard();

                dashboard.replace_new_pane(pane);

                Task::none()
            },
            Message::ResetCurrentLayout => {
                let new_dashboard = Dashboard::empty();

                self.layouts.insert(self.last_active_layout, new_dashboard);

                Task::perform(
                    async {},
                    move |_| Message::Notification(
                        Notification::Info("Layout reset".to_string())
                    )
                )
            },
            Message::LayoutSelected(layout_id) => {
                self.last_active_layout = layout_id;
            
                let mut tasks = vec![];

                self.pane_streams = self.get_dashboard().get_all_diff_streams();

                tasks.push(
                    Task::perform(
                        async {},
                        move |_| Message::Notification(Notification::Info("Fetching data...".to_string()))
                    )
                );

                tasks.extend(
                    klines_fetch_all_task(&self.pane_streams)
                );
                tasks.extend(
                    ticksize_fetch_all_task(&self.pane_streams)
                );

                Task::batch(tasks)
            },
            Message::FetchDistributeKlines(stream_type, klines) => {
                let dashboard = self.get_mut_dashboard();

                match klines {
                    Ok(klines) => {
                        if let Err(err) = dashboard.find_and_insert_klines(&stream_type, &klines) {
                            log::error!("{err}");
                        }
                    },
                    Err(err) => {
                        log::error!("{err}");
                    }
                }

                Task::none()
            },  
            Message::FetchDistributeTicks(stream_type, min_tick_size) => {
                let dashboard = self.get_mut_dashboard();

                match min_tick_size {
                    Ok(ticksize) => {
                        if let Err(err) = dashboard.find_and_insert_ticksizes(&stream_type, ticksize) {
                            log::error!("{err}");
                        }
                    },
                    Err(err) => {
                        log::error!("{err}");
                    }
                }

                Task::none()
            },
            Message::Dashboard(message) => {
                let dashboard = self.get_mut_dashboard();
                
                let command = dashboard.update(
                    message,
                );

                Task::batch(vec![
                    command.map(Message::Dashboard),
                ])
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let dashboard = self.get_dashboard();

        let layout_lock_button = button(
            container(
                if dashboard.layout_lock { 
                    text(char::from(Icon::Locked).to_string()).font(ICON_FONT) 
                } else { 
                    text(char::from(Icon::Unlocked).to_string()).font(ICON_FONT) 
                })
                .width(25)
                .center_x(iced::Pixels(20.0))
            )
            .on_press(Message::ToggleLayoutLock);

        let layout_modal_button = button(
            container(
                text(char::from(Icon::Layout).to_string()).font(ICON_FONT))
                .width(25)
                .center_x(iced::Pixels(20.0))
            )
            .on_press(Message::ShowLayoutModal);

        let pane_modal_button = button(
            container(
                text(char::from(Icon::Layout).to_string()).font(ICON_FONT))
                .width(25)
                .center_x(iced::Pixels(20.0))
            )
            .on_press(Message::ShowPanesModal);

        let layout_controls = Row::new()
            .spacing(10)
            .align_y(Alignment::Center)
            .push(
                tooltip(
                    pane_modal_button, 
                    "Manage Panes", tooltip::Position::Bottom
                ).style(style::tooltip)
            )
            .push(
                tooltip(
                    layout_modal_button, 
                    "Manage Layouts", tooltip::Position::Bottom
                ).style(style::tooltip)
            )
            .push(
                tooltip(
                    layout_lock_button, 
                    "Layout Lock", tooltip::Position::Bottom
                ).style(style::tooltip)
            );

        let mut ws_controls = Row::new()
            .spacing(10)
            .align_y(Alignment::Center);

        if let Some(notification) = &self.notification {
            match notification {
                Notification::Info(string) => {
                    ws_controls = ws_controls.push(
                        container(
                            Column::new()
                                .padding(4)
                                .push(
                                    Text::new(format!("{string}"))
                                        .size(14)
                                )
                        ).style(style::notification)
                    );
                },
                Notification::Error(string) => {
                    ws_controls = ws_controls.push(
                        container(
                            Column::new()
                                .padding(4)
                                .push(
                                    Text::new(format!("err: {string}"))
                                        .size(14)
                                )
                        ).style(style::notification)
                    );
                },
                Notification::Warn(string) => {
                    ws_controls = ws_controls.push(
                        container(
                            Column::new()
                                .padding(4)
                                .push(
                                    Text::new(format!("warn: {string}"))
                                        .size(14)
                                )
                        ).style(style::notification)
                    );
                },
            }
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
            .push(
                dashboard.view().map(Message::Dashboard)
            );

        if self.show_layout_modal {
            let layout_picklist = pick_list(
                &LayoutId::ALL[..],
                Some(self.last_active_layout),
                move |layout: LayoutId| Message::LayoutSelected(layout)
            );

            let layout_modal = container(
                Column::new()
                    .spacing(16)
                    .align_x(Alignment::Center)
                    .push(
                        Column::new()
                            .align_x(Alignment::Center)
                            .push(Text::new("Layouts"))
                            .padding([8, 0])
                            .spacing(8)
                            .push(
                                Row::new()
                                    .push(
                                        Row::new()
                                        .spacing(8)
                                        .push(
                                            tooltip(
                                                button(Text::new("Reset"))
                                                .on_press(Message::ResetCurrentLayout),
                                                "Reset current layout", 
                                                tooltip::Position::Top
                                            ).style(style::tooltip)
                                        )
                                        .push(
                                            layout_picklist
                                            .style(style::picklist_primary)
                                            .menu_style(style::picklist_menu_primary)
                                        )
                                        .push(
                                            tooltip(
                                                button(Text::new("i")).style(style::button_for_info),
                                                "Layouts won't be saved if app exited abruptly", 
                                                tooltip::Position::Top
                                            ).style(style::tooltip)
                                        )                         
                                    )
                            )
                    )       
                    .push(
                        button("Close")
                            .on_press(Message::HideLayoutModal)
                    )
            )
            .width(Length::Shrink)
            .padding(20)
            .style(style::chart_modal);

            modal(content, layout_modal, Message::HideLayoutModal)
        } else {
            content 
                .into()
        }  
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut all_subscriptions = Vec::new();
    
        for (exchange, stream) in &self.pane_streams {
            let mut depth_streams: Vec<Subscription<Message>> = Vec::new();
            let mut kline_streams: Vec<(Ticker, Timeframe)> = Vec::new();
    
            for stream_types in stream.values() {
                for stream_type in stream_types {
                    match stream_type {
                        StreamType::Kline { ticker, timeframe, .. } => {
                            kline_streams.push((*ticker, *timeframe));
                        },
                        StreamType::DepthAndTrades { ticker, .. } => {
                            let ticker = *ticker;

                            let depth_stream = match exchange {
                                Exchange::BinanceFutures => {
                                    Subscription::run_with_id(ticker, binance::market_data::connect_market_stream(ticker))
                                        .map(|event| Message::MarketWsEvent(MarketEvents::Binance(event)))
                                },
                                Exchange::BybitLinear => {
                                    Subscription::run_with_id(ticker, bybit::market_data::connect_market_stream(ticker))
                                        .map(|event| Message::MarketWsEvent(MarketEvents::Bybit(event)))
                                },
                            };
                            depth_streams.push(depth_stream);
                        },
                        _ => {}
                    }
                }
            }
    
            if !kline_streams.is_empty() {
                let kline_streams_id = kline_streams.clone();

                let kline_subscription = match exchange {
                    Exchange::BinanceFutures => {
                        Subscription::run_with_id(kline_streams_id, binance::market_data::connect_kline_stream(kline_streams))
                            .map(|event| Message::MarketWsEvent(MarketEvents::Binance(event)))
                    },
                    Exchange::BybitLinear => {
                        Subscription::run_with_id(kline_streams_id, bybit::market_data::connect_kline_stream(kline_streams))
                            .map(|event| Message::MarketWsEvent(MarketEvents::Bybit(event)))
                    },
                };
                all_subscriptions.push(kline_subscription);
            }
    
            if !depth_streams.is_empty() {
                all_subscriptions.push(Subscription::batch(depth_streams));
            }
        }

        all_subscriptions.push(events().map(Message::Event));
    
        Subscription::batch(all_subscriptions)
    }    
    
    fn get_mut_dashboard(&mut self) -> &mut Dashboard {
        self.layouts
            .get_mut(&self.last_active_layout)
            .expect("No active layout")
    }

    fn get_dashboard(&self) -> &Dashboard {
        self.layouts
            .get(&self.last_active_layout)
            .expect("No active layout")
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

fn klines_fetch_all_task(stream_types: &HashMap<Exchange, HashMap<Ticker, HashSet<StreamType>>>) -> Vec<Task<Message>> {
    let mut tasks: Vec<Task<Message>> = vec![];

    for (exchange, stream) in stream_types {
        let mut kline_fetches = Vec::new();

        for stream_types in stream.values() {
            for stream_type in stream_types {
                match stream_type {
                    StreamType::Kline { ticker, timeframe, .. } => {
                        kline_fetches.push((*ticker, *timeframe));
                    },
                    _ => {}
                }
            }
        }

        for (ticker, timeframe) in kline_fetches {
            let ticker = ticker;
            let timeframe = timeframe;
            let exchange = *exchange;

            match exchange {
                Exchange::BinanceFutures => {
                    let fetch_klines = Task::perform(
                        binance::market_data::fetch_klines(ticker, timeframe)
                            .map_err(|err| format!("{err}")),
                        move |klines| Message::FetchDistributeKlines(
                            StreamType::Kline { exchange, ticker, timeframe }, klines
                        )
                    );
                    tasks.push(fetch_klines);
                },
                Exchange::BybitLinear => {
                    let fetch_klines = Task::perform(
                        bybit::market_data::fetch_klines(ticker, timeframe)
                            .map_err(|err| format!("{err}")),
                        move |klines| Message::FetchDistributeKlines(
                            StreamType::Kline { exchange, ticker, timeframe }, klines
                        )
                    );
                    tasks.push(fetch_klines);
                }
            }
        }
    }

    tasks
}

fn ticksize_fetch_all_task(stream_types: &HashMap<Exchange, HashMap<Ticker, HashSet<StreamType>>>) -> Vec<Task<Message>> {
    let mut tasks: Vec<Task<Message>> = vec![];

    for (exchange, stream) in stream_types {
        let mut ticksize_fetches = Vec::new();

        for stream_types in stream.values() {
            for stream_type in stream_types {
                match stream_type {
                    StreamType::DepthAndTrades { ticker, .. } => {
                        ticksize_fetches.push(*ticker);
                    },
                    _ => {}
                }
            }
        }

        for ticker in ticksize_fetches {
            let ticker = ticker;
            let exchange = *exchange;

            match exchange {
                Exchange::BinanceFutures => {
                    let fetch_ticksize = Task::perform(
                        binance::market_data::fetch_ticksize(ticker)
                            .map_err(|err| format!("{err}")),
                        move |ticksize| Message::FetchDistributeTicks(
                            StreamType::DepthAndTrades { exchange, ticker }, ticksize
                        )
                    );
                    tasks.push(fetch_ticksize);
                },
                Exchange::BybitLinear => {
                    let fetch_ticksize = Task::perform(
                        bybit::market_data::fetch_ticksize(ticker)
                            .map_err(|err| format!("{err}")),
                        move |ticksize| Message::FetchDistributeTicks(
                            StreamType::DepthAndTrades { exchange, ticker }, ticksize
                        )
                    );
                    tasks.push(fetch_ticksize);
                }
            }
        }
    }

    tasks
}

fn create_fetch_klines_task(
    stream: StreamType,
    pane_id: Uuid,
) -> Task<Message> {
    match stream {
        StreamType::Kline { exchange, ticker, timeframe } => {
            match exchange {
                Exchange::BinanceFutures => Task::perform(
                    binance::market_data::fetch_klines(ticker, timeframe)
                        .map_err(|err| format!("{err}")),
                    move |klines| Message::FetchEvent(klines, stream, pane_id),
                ),
                Exchange::BybitLinear => Task::perform(
                    bybit::market_data::fetch_klines(ticker, timeframe)
                        .map_err(|err| format!("{err}")),
                    move |klines| Message::FetchEvent(klines, stream, pane_id),
                ),
            }
        },
        _ => Task::none(),
    }
}

fn create_fetch_ticksize_task(
    exchange: &Exchange,
    ticker: &Ticker,
    pane_id: Uuid,
) -> Task<Message> {
    match exchange {
        Exchange::BinanceFutures => Task::perform(
            binance::market_data::fetch_ticksize(*ticker),
            move |result| match result {
                Ok(ticksize) => Message::SetMinTickSize(ticksize, pane_id),
                Err(err) => Message::ErrorOccurred(Error::FetchError(err.to_string())),
            },
        ),
        Exchange::BybitLinear => Task::perform(
            bybit::market_data::fetch_ticksize(*ticker),
            move |result| match result {
                Ok(ticksize) => Message::SetMinTickSize(ticksize, pane_id),
                Err(err) => Message::ErrorOccurred(Error::FetchError(err.to_string())),
            },
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    CloseRequested(window::Id),
    Copy,
    Escape,
    Home,
    End,
}

pub fn events() -> Subscription<Event> {
    iced::event::listen_with(filtered_events)
}

fn filtered_events(
    event: iced::Event,
    _status: iced::event::Status,
    window: window::Id,
) -> Option<Event> {
    match &event {
        iced::Event::Window(window::Event::CloseRequested) => Some(Event::CloseRequested(window)),
        _ => None,
    }
}