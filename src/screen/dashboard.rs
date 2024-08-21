pub mod pane;

use futures::TryFutureExt;
use pane::SerializablePane;
pub use pane::{Uuid, PaneState, PaneContent, PaneSettings};
use serde::{Deserialize, Serialize};

use crate::{
    charts::{candlestick::CandlestickChart, footprint::FootprintChart, heatmap::HeatmapChart, timeandsales::TimeAndSales, Message as ChartMessage}, data_providers::{
        binance, bybit, Depth, Exchange, Kline, TickMultiplier, Ticker, Timeframe, Trade
    }, modal, style, StreamType
};

use super::{Error, Notification};

use std::{collections::{HashMap, HashSet}, rc::Rc};
use iced::{widget::{button, container, pane_grid::{self, Configuration}, Column, PaneGrid, Text}, window, Alignment, Element, Length, Point, Size, Task};

#[derive(Debug, Clone)]
pub enum Message {
    Pane(pane::Message),
    ErrorOccurred(Error),
    Notification(Notification),
    FetchEvent(Result<Vec<Kline>, String>, StreamType, Uuid),
    FetchDistributeKlines(StreamType, Result<Vec<Kline>, String>),
    FetchDistributeTicks(StreamType, Result<f32, String>),
    FetchForLayout,
}

pub struct Dashboard {
    pub panes: pane_grid::State<PaneState>,
    pub focus: Option<pane_grid::Pane>,
    pub layout_lock: bool,
    pub pane_streams: HashMap<Exchange, HashMap<Ticker, HashSet<StreamType>>>,
    pub notification: Option<Notification>,
}
impl Dashboard {
    pub fn empty() -> Self {
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
        
        Self { 
            panes: pane_grid::State::with_configuration(pane_config),
            focus: None,
            layout_lock: false,
            pane_streams: HashMap::new(),
            notification: None,
        }
    }

    pub fn from_config(panes: Configuration<PaneState>) -> Self {
        Self {
            panes: pane_grid::State::with_configuration(panes),
            focus: None,
            layout_lock: false,
            pane_streams: HashMap::new(),
            notification: None,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Pane(message) => {
                match message {
                    pane::Message::PaneClicked(pane_id) => {
                        self.focus = Some(pane_id);
                    },
                    pane::Message::PaneResized(pane_grid::ResizeEvent { split, ratio })=> {
                        self.panes.resize(split, ratio);
                    },
                    pane::Message::PaneDragged(event) => {
                        match event {
                            pane_grid::DragEvent::Dropped { pane, target } => {
                                self.panes.drop(pane, target);

                                self.focus = None;
                            },
                            _ => {}
                        }
                    },
                    pane::Message::SplitPane(axis, pane) => {        
                        let focus_pane = if let Some((new_pane, _)) = 
                            self.panes.split(axis, pane, PaneState::new(Uuid::new_v4(), vec![], PaneSettings::default())) {
                                    Some(new_pane)
                                } else {
                                    None
                                };
        
                        if Some(focus_pane).is_some() {
                            self.focus = focus_pane;
                        }
                    },
                    pane::Message::ClosePane(pane) => {
                        if let Some((_, sibling)) = self.panes.close(pane) {
                            self.focus = Some(sibling);
                        }
                    },
                    pane::Message::MaximizePane(pane) => {
                        self.panes.maximize(pane);
                    },
                    pane::Message::Restore => {
                        self.panes.restore();
                    },
                    pane::Message::TickerSelected(ticker, pane_id) => {
                        if let Ok(settings) = self.get_pane_settings_mut(pane_id) {
                            settings.selected_ticker = Some(ticker);
                        }
                    },
                    pane::Message::ExchangeSelected(exchange, pane_id) => {
                        if let Ok(settings) = self.get_pane_settings_mut(pane_id) {
                            settings.selected_exchange = Some(exchange);
                        }
                    },
                    pane::Message::ReplacePane(pane_id) => {
                        self.replace_new_pane(pane_id);
                    },
                    pane::Message::ShowModal(pane_id) => {
                        if let Some(pane) = self.panes.get_mut(pane_id) {
                            pane.show_modal = true;
                        };
                    },
                    pane::Message::HideModal(pane_id) => {
                        for (_, pane_state) in self.panes.iter_mut() {
                            if pane_state.id == pane_id {
                                pane_state.show_modal = false;
                            }
                        }
                    },
                    pane::Message::ChartUserUpdate(message, pane_id) => {
                        match self.update_chart_state(pane_id, message) {
                            Ok(_) => return Task::none(),
                            Err(err) => {      
                                return Task::perform(
                                    async { err },
                                    move |err: Error| Message::ErrorOccurred(err)
                                );
                            }
                        }
                    },
                    pane::Message::SliderChanged(pane_id, value) => {
                        match self.set_pane_size_filter(pane_id, value) {
                            Ok(_) => {
                                log::info!("Size filter set to {value}");

                                return Task::none()
                            }
                            Err(err) => {
                                return Task::perform(
                                    async { err },
                                    move |err: Error| Message::ErrorOccurred(err)
                                )
                            }
                        }
                    },
                    pane::Message::PaneContentSelected(content, pane_id, pane_stream) => {        
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
                        if let Err(err) = self.set_pane_content(pane_id, pane_content) {
                            log::error!("Failed to set pane content: {}", err);
                        } else {
                            log::info!("Pane content set: {content}");
                        }
                        
                        if let Err(err) = self.set_pane_stream(pane_id, pane_stream.to_vec()) {
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
                        }
                        
                        return Task::batch(tasks)
                    },
                    pane::Message::TimeframeSelected(timeframe, pane_id) => {    
                        let mut tasks = vec![];
                
                        match self.set_pane_timeframe(pane_id, timeframe) {
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
        
                                    self.pane_streams = self.get_all_diff_streams();
                                }
                            },
                            Err(err) => {
                                tasks.push(Task::perform(
                                    async { err },
                                    move |err: Error| Message::ErrorOccurred(err)
                                ));
                            }
                        }
        
                        return Task::batch(tasks)
                    },
                    pane::Message::TicksizeSelected(tick_multiply, pane_id) => {                        
                        match self.set_pane_ticksize(pane_id, tick_multiply) {
                            Ok(_) => {
                            },
                            Err(err) => {            
                                return Task::perform(
                                    async { err },
                                    move |err: Error| Message::ErrorOccurred(err)
                                )
                            }
                        }
                    },
                    pane::Message::SetMinTickSize(pane_id, ticksize) => {        
                        match self.get_pane_settings_mut(pane_id) {
                            Ok(pane_settings) => {
                                pane_settings.min_tick_size = Some(ticksize);
                            },
                            Err(err) => {
                                return Task::perform(
                                    async { err },
                                    move |err: Error| Message::ErrorOccurred(err)
                                )
                            }
                        }
                    },
                }
            },
            Message::ErrorOccurred(err) => {
                dbg!(err);
            },
            Message::Notification(notification) => {
                dbg!(notification);
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
               
                match klines {
                    Ok(klines) => {
                        if let StreamType::Kline { .. } = pane_stream {
                            self.insert_klines_vec(&pane_stream, &klines, pane_id);
                        } else {
                            log::error!("Invalid stream type for klines: {pane_stream:?}");
                        }
                    },
                    Err(err) => {
                        return Task::perform(
                            async { err },
                            move |err: String| Message::ErrorOccurred(Error::FetchError(err))
                        )
                    }
                }
            },
            Message::FetchDistributeKlines(stream_type, klines) => {
                match klines {
                    Ok(klines) => {
                        if let Err(err) = self.find_and_insert_klines(&stream_type, &klines) {
                            log::error!("{err}");
                        }
                    },
                    Err(err) => {
                        log::error!("{err}");
                    }
                }
            },  
            Message::FetchDistributeTicks(stream_type, min_tick_size) => {
                match min_tick_size {
                    Ok(ticksize) => {
                        if let Err(err) = self.find_and_insert_ticksizes(&stream_type, ticksize) {
                            log::error!("{err}");
                        }
                    },
                    Err(err) => {
                        log::error!("{err}");
                    }
                }
            },
            Message::FetchForLayout => {
                let mut tasks = vec![];

                let pane_streams = self.get_all_diff_streams();

                tasks.extend(
                    klines_fetch_all_task(&pane_streams)
                );
                tasks.extend(
                    ticksize_fetch_all_task(&pane_streams)
                );
 
                return Task::batch(tasks)
            },
        }

        Task::none()
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let focus = self.focus;
        let pane_locked = self.layout_lock;
        
        let mut pane_grid = PaneGrid::new(&self.panes, |id, pane, maximized| {
            let is_focused = !pane_locked && focus == Some(id);
            pane.view(
                id,
                self.panes.len(),
                is_focused,
                maximized,
            )
        })
        .spacing(4);
    
        if !pane_locked {
            pane_grid = pane_grid
                .on_click(pane::Message::PaneClicked)
                .on_resize(6, pane::Message::PaneResized)
                .on_drag(pane::Message::PaneDragged);
        }
    
        let pane_grid: Element<_> = pane_grid.into();

        let pane_grid = container(pane_grid.map(Message::Pane))
            .width(Length::Fill)
            .height(Length::Fill);

        pane_grid.into()
    }

    pub fn layout_changed(&mut self) -> Task<Message> {
        self.pane_streams = self.get_all_diff_streams();

        Task::perform(
            async {},
            move |_| Message::FetchForLayout
        )
    }

    fn replace_new_pane(&mut self, pane: pane_grid::Pane) {
        if let Some(pane) = self.panes.get_mut(pane) {
            *pane = PaneState::new(Uuid::new_v4(), vec![], PaneSettings::default());
        }
    }

    fn get_pane_settings_mut(&mut self, pane_id: Uuid) -> Result<&mut PaneSettings, Error> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                return Ok(&mut pane_state.settings);
            }
        }
        Err(Error::UnknownError("No pane found".to_string()))
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

    fn set_pane_stream(&mut self, pane_id: Uuid, stream: Vec<StreamType>) -> Result<(), &str> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                pane_state.stream = stream;

                return Ok(());
            }
        }
        Err("No pane found")
    }

    fn set_pane_ticksize(&mut self, pane_id: Uuid, new_tick_multiply: TickMultiplier) -> Result<(), Error> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                pane_state.settings.tick_multiply = Some(new_tick_multiply);

                if let Some(min_tick_size) = pane_state.settings.min_tick_size {
                    match pane_state.content {
                        PaneContent::Footprint(ref mut chart) => {
                            chart.change_tick_size(
                                new_tick_multiply.multiply_with_min_tick_size(min_tick_size)
                            );
                            
                            return Ok(());
                        },
                        PaneContent::Heatmap(ref mut chart) => {
                            chart.change_tick_size(
                                new_tick_multiply.multiply_with_min_tick_size(min_tick_size)
                            );
                            
                            return Ok(());
                        },
                        _ => {
                            return Err(Error::UnknownError("No chart found to change ticksize".to_string()));
                        }
                    }
                } else {
                    return Err(Error::UnknownError("No min tick size found".to_string()));
                }
            }
        }
        Err(Error::UnknownError("No pane found to change ticksize".to_string()))
    }
    
    fn set_pane_timeframe(&mut self, pane_id: Uuid, new_timeframe: Timeframe) -> Result<&StreamType, Error> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                pane_state.settings.selected_timeframe = Some(new_timeframe);

                for stream_type in pane_state.stream.iter_mut() {
                    match stream_type {
                        StreamType::Kline { timeframe, .. } => {
                            *timeframe = new_timeframe;

                            match pane_state.content {
                                PaneContent::Candlestick(_) => {
                                    return Ok(stream_type);
                                },
                                PaneContent::Footprint(_) => {
                                    return Ok(stream_type);
                                },
                                _ => {}
                            }
                        },
                        _ => {}
                    }
                }
            }
        }
        Err(Error::UnknownError("No pane found to change tiemframe".to_string()))
    }

    fn set_pane_size_filter(&mut self, pane_id: Uuid, new_size_filter: f32) -> Result<(), Error> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                pane_state.settings.trade_size_filter = Some(new_size_filter);

                match pane_state.content {
                    PaneContent::Heatmap(ref mut chart) => {
                        chart.set_size_filter(new_size_filter);

                        return Ok(());
                    },
                    PaneContent::TimeAndSales(ref mut chart) => {
                        chart.set_size_filter(new_size_filter);
                        
                        return Ok(());
                    },
                    _ => {
                        return Err(Error::UnknownError("No chart found".to_string()));
                    }
                }
            }
        }
        Err(Error::UnknownError("No pane found".to_string()))
    }

    pub fn find_and_insert_ticksizes(&mut self, stream_type: &StreamType, tick_sizes: f32) -> Result<(), &str> {
        let mut found_match = false;

        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.matches_stream(stream_type) {
                match &mut pane_state.content {
                    PaneContent::Footprint(_) => {
                        pane_state.settings.min_tick_size = Some(tick_sizes);

                        found_match = true;
                    },
                    PaneContent::Heatmap(_) => {
                        pane_state.settings.min_tick_size = Some(tick_sizes);

                        found_match = true;
                    },
                    _ => {}
                }
            }
        }

        if found_match {
            Ok(())
        } else {
            Err("No matching pane found for the stream")
        }
    }

    pub fn find_and_insert_klines(&mut self, stream_type: &StreamType, klines: &Vec<Kline>) -> Result<(), &str> {
        let mut found_match = false;

        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.matches_stream(stream_type) {
                match stream_type {
                    StreamType::Kline { timeframe, .. } => {
                        let timeframe_u16 = timeframe.to_minutes();

                        match &mut pane_state.content {
                            PaneContent::Candlestick(chart) => {
                                *chart = CandlestickChart::new(klines.to_vec(), timeframe_u16);

                                found_match = true;
                            },
                            PaneContent::Footprint(chart) => {
                                let raw_trades = chart.get_raw_trades();

                                let tick_size = chart.get_tick_size();

                                *chart = FootprintChart::new(timeframe_u16, tick_size, klines.to_vec(), raw_trades);

                                found_match = true;
                            },
                            _ => {}
                        }
                    },
                    _ => {}
                }
            }
        }

        if found_match {
            Ok(())
        } else {
            Err("No matching pane found for the stream")
        }
    }

    pub fn insert_klines_vec(&mut self, stream_type: &StreamType, klines: &Vec<Kline>, pane_id: Uuid) {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                match stream_type {
                    StreamType::Kline { timeframe, .. } => {
                        let timeframe_u16 = timeframe.to_minutes();

                        match &mut pane_state.content {
                            PaneContent::Candlestick(chart) => {
                                *chart = CandlestickChart::new(klines.to_vec(), timeframe_u16);
                            },
                            PaneContent::Footprint(chart) => {
                                let raw_trades = chart.get_raw_trades();

                                let tick_size = chart.get_tick_size();

                                *chart = FootprintChart::new(timeframe_u16, tick_size, klines.to_vec(), raw_trades);
                            },
                            _ => {}
                        }
                    },
                    _ => {}
                }
            }
        }
    }

    pub fn update_latest_klines(&mut self, stream_type: &StreamType, kline: &Kline) -> Result<(), &str> {
        let mut found_match = false;
    
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.matches_stream(stream_type) {
                match &mut pane_state.content {
                    PaneContent::Candlestick(chart) => chart.update_latest_kline(kline),
                    PaneContent::Footprint(chart) => chart.update_latest_kline(kline),
                    _ => {}
                }
                found_match = true;
            }
        }
    
        if found_match {
            Ok(())
        } else {
            self.pane_streams = self.get_all_diff_streams();

            Err("No matching pane found for the stream")
        }
    }

    pub fn update_depth_and_trades(&mut self, stream_type: StreamType, depth_update_t: i64, depth: Depth, trades_buffer: Vec<Trade>) -> Result<(), &str> {
        let mut found_match = false;
        
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

                found_match = true;
            }
        }

        if found_match {
            Ok(())
        } else {
            self.pane_streams = self.get_all_diff_streams();

            Err("No matching pane found for the stream")
        }
    }

    fn update_chart_state(&mut self, pane_id: Uuid, chart_message: ChartMessage) -> Result<(), Error> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                match pane_state.content {
                    PaneContent::Heatmap(ref mut chart) => {
                        chart.update(&chart_message);

                        return Ok(());
                    },
                    PaneContent::Footprint(ref mut chart) => {
                        chart.update(&chart_message);

                        return Ok(());
                    },
                    PaneContent::Candlestick(ref mut chart) => {
                        chart.update(&chart_message);

                        return Ok(());
                    },
                    _ => {
                        return Err(Error::UnknownError("No chart found".to_string()));
                    }
                }
            }
        }
        Err(Error::UnknownError("No pane found to update its state".to_string()))
    }

    pub fn get_all_diff_streams(&mut self) -> HashMap<Exchange, HashMap<Ticker, HashSet<StreamType>>> {
        let mut pane_streams = HashMap::new();

        for (_, pane_state) in self.panes.iter() {
            for stream_type in &pane_state.stream {
                match stream_type {
                    StreamType::Kline { exchange, ticker, timeframe } => {
                        let exchange = *exchange;
                        let ticker = *ticker;
                        let timeframe = *timeframe;

                        let exchange_map = pane_streams.entry(exchange).or_insert(HashMap::new());
                        let ticker_map = exchange_map.entry(ticker).or_insert(HashSet::new());
                        ticker_map.insert(StreamType::Kline { exchange, ticker, timeframe });
                    },
                    StreamType::DepthAndTrades { exchange, ticker } => {
                        let exchange = *exchange;
                        let ticker = *ticker;

                        let exchange_map = pane_streams.entry(exchange).or_insert(HashMap::new());
                        let ticker_map = exchange_map.entry(ticker).or_insert(HashSet::new());
                        ticker_map.insert(StreamType::DepthAndTrades { exchange, ticker });
                    },
                    _ => {}
                }
            }
        }
        self.pane_streams = pane_streams.clone();

        pane_streams
    }
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
                Ok(ticksize) => Message::Pane(pane::Message::SetMinTickSize(pane_id, ticksize)),
                Err(err) => Message::ErrorOccurred(Error::FetchError(err.to_string())),
            },
        ),
        Exchange::BybitLinear => Task::perform(
            bybit::market_data::fetch_ticksize(*ticker),
            move |result| match result {
                Ok(ticksize) => Message::Pane(pane::Message::SetMinTickSize(pane_id, ticksize)),
                Err(err) => Message::ErrorOccurred(Error::FetchError(err.to_string())),
            },
        ),
    }
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

impl Default for Dashboard {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SerializableDashboard {
    pub pane: SerializablePane,
}

impl<'a> From<&'a Dashboard> for SerializableDashboard {
    fn from(dashboard: &'a Dashboard) -> Self {
        use pane_grid::Node;

        fn from_layout(panes: &pane_grid::State<PaneState>, node: pane_grid::Node) -> SerializablePane {
            match node {
                Node::Split {
                    axis, ratio, a, b, ..
                } => SerializablePane::Split {
                    axis: match axis {
                        pane_grid::Axis::Horizontal => pane::Axis::Horizontal,
                        pane_grid::Axis::Vertical => pane::Axis::Vertical,
                    },
                    ratio,
                    a: Box::new(from_layout(panes, *a)),
                    b: Box::new(from_layout(panes, *b)),
                },
                Node::Pane(pane) => panes
                    .get(pane)
                    .map(SerializablePane::from)
                    .unwrap_or(SerializablePane::Starter),
            }
        }

        let layout = dashboard.panes.layout().clone();

        SerializableDashboard {
            pane: from_layout(&dashboard.panes, layout),
        }
    }
}

impl Default for SerializableDashboard {
    fn default() -> Self {
        Self {
            pane: SerializablePane::Starter,
        }
    }
}