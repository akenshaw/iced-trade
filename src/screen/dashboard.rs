pub mod pane;

use pane::SerializablePane;
pub use pane::{Uuid, PaneState, PaneContent, PaneSettings};
use serde::{Deserialize, Serialize};

use crate::{
    charts::{candlestick::CandlestickChart, footprint::FootprintChart, heatmap::HeatmapChart, Message as ChartMessage}, data_providers::{
        Depth, Exchange, Kline, TickMultiplier, Ticker, Timeframe, Trade
    }, modal, style::{self, Icon, ICON_FONT}, StreamType
};

use super::{Error, Notification};

use std::{collections::{HashMap, HashSet}, io::Read, rc::Rc};
use iced::{widget::{button, container, pane_grid::{self, Configuration}, pick_list, row, text, tooltip, Column, PaneGrid, Row, Text}, window, Alignment, Element, Length, Point, Renderer, Size, Task};

#[derive(Debug, Clone)]
pub enum Message {
    Pane(pane::Message),
    Close(window::Id),
    DashboardSaved(Result<(), Error>),
    CloseContextMenu(bool),
    HidePanesModal,
}

pub struct Dashboard {
    pub panes: pane_grid::State<PaneState>,
    pub focus: Option<pane_grid::Pane>,
    pub layout_lock: bool,
    pub show_panes_modal: bool,
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
            show_panes_modal: false,
        }
    }

    pub fn from_config(panes: Configuration<PaneState>) -> Self {
        Self {
            panes: pane_grid::State::with_configuration(panes),
            focus: None,
            layout_lock: false,
            show_panes_modal: false,
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
                    _ => {}
                }
            }
            Message::Close(window_id) => {
                self.focus = None;
            },
            Message::DashboardSaved(_) => {
                self.show_panes_modal = false;
            },
            Message::CloseContextMenu(_) => {
                self.show_panes_modal = false;
            },
            Message::HidePanesModal => {
                self.show_panes_modal = false;
            },
        }

        Task::none()
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let focus = self.focus;

        let pane_grid: Element<_> = PaneGrid::new(&self.panes, |id, pane, maximized| {
            let is_focused;

            if self.layout_lock {
                is_focused = false;
            } else {
                is_focused = focus == Some(id);
            }

            let panes = self.panes.len();
            
            pane.view(
                id,
                panes,
                is_focused,
                maximized,
            )
        })
        .on_click(pane::Message::PaneClicked)
        .on_resize(6, pane::Message::PaneResized)
        .on_drag(pane::Message::PaneDragged)
        .spacing(4)
        .into();

        let pane_grid = container(pane_grid.map(Message::Pane))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8);

        if self.show_panes_modal {
            let mut add_pane_button = button("Split selected pane").width(iced::Pixels(200.0));

            let mut replace_pane_button = button("Replace selected pane").width(iced::Pixels(200.0));

            if self.focus.is_some() {
                replace_pane_button = replace_pane_button.on_press(
                    Message::Pane(pane::Message::ReplacePane(
                        self.focus
                            .unwrap_or_else(|| { *self.panes.iter().next().unwrap().0 })
                    ))
                );

                add_pane_button = add_pane_button.on_press(
                    Message::Pane(pane::Message::SplitPane(
                        pane_grid::Axis::Horizontal, 
                        self.focus
                            .unwrap_or_else(|| { *self.panes.iter().next().unwrap().0 })
                    ))
                );
            }

            let layout_modal = container(
                Column::new()
                    .spacing(16)
                    .align_x(Alignment::Center)
                    .push(
                        Column::new()
                            .align_x(Alignment::Center)
                            .push(Text::new("Panes"))
                            .padding([8, 0])
                            .spacing(8)
                            .push(add_pane_button)
                            .push(replace_pane_button)
                    )       
                    .push(
                        button("Close")
                            .on_press(Message::HidePanesModal)
                    )
            )
            .width(Length::Shrink)
            .padding(20)
            .style(style::chart_modal);

            modal(pane_grid, layout_modal, Message::HidePanesModal)
        } else {
            pane_grid.into()
        }
    }

    pub fn replace_new_pane(&mut self, pane: pane_grid::Pane) {
        if let Some(pane) = self.panes.get_mut(pane) {
            *pane = PaneState::new(Uuid::new_v4(), vec![], PaneSettings::default());
        }
    }

    pub fn get_pane_settings_mut(&mut self, pane_id: Uuid) -> Result<&mut PaneSettings, Error> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                return Ok(&mut pane_state.settings);
            }
        }
        Err(Error::UnknownError("No pane found".to_string()))
    }

    pub fn set_pane_content(&mut self, pane_id: Uuid, content: PaneContent) -> Result<(), &str> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                pane_state.content = content;

                return Ok(());
            }
        }
        Err("No pane found")
    }

    pub fn set_pane_stream(&mut self, pane_id: Uuid, stream: Vec<StreamType>) -> Result<(), &str> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                pane_state.stream = stream;

                return Ok(());
            }
        }
        Err("No pane found")
    }

    pub fn set_pane_ticksize(&mut self, pane_id: Uuid, new_tick_multiply: TickMultiplier) -> Result<(), Error> {
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
    
    pub fn set_pane_timeframe(&mut self, pane_id: Uuid, new_timeframe: Timeframe) -> Result<&StreamType, Error> {
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

    pub fn set_pane_size_filter(&mut self, pane_id: Uuid, new_size_filter: f32) -> Result<(), Error> {
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
            Err("No matching pane found for the stream")
        }
    }

    pub fn update_chart_state(&mut self, pane_id: Uuid, ChartMessage: ChartMessage) -> Result<(), Error> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                match pane_state.content {
                    PaneContent::Heatmap(ref mut chart) => {
                        chart.update(&ChartMessage);

                        return Ok(());
                    },
                    PaneContent::Footprint(ref mut chart) => {
                        chart.update(&ChartMessage);

                        return Ok(());
                    },
                    PaneContent::Candlestick(ref mut chart) => {
                        chart.update(&ChartMessage);

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

    pub fn get_all_diff_streams(&self) -> HashMap<Exchange, HashMap<Ticker, HashSet<StreamType>>> {
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

        pane_streams
    }
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

pub struct SavedState {
    pub layouts: HashMap<LayoutId, Dashboard>,
    pub last_active_layout: LayoutId,
    pub window_size: Option<(f32, f32)>,
    pub window_position: Option<(f32, f32)>,
}
impl Default for SavedState {
    fn default() -> Self {
        let mut layouts = HashMap::new();
        layouts.insert(LayoutId::Layout1, Dashboard::default());
        layouts.insert(LayoutId::Layout2, Dashboard::default());
        layouts.insert(LayoutId::Layout3, Dashboard::default());
        layouts.insert(LayoutId::Layout4, Dashboard::default());
        
        SavedState {
            layouts,
            last_active_layout: LayoutId::Layout1,
            window_size: None,
            window_position: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum LayoutId {
    Layout1,
    Layout2,
    Layout3,
    Layout4,
}
impl std::fmt::Display for LayoutId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutId::Layout1 => write!(f, "Layout 1"),
            LayoutId::Layout2 => write!(f, "Layout 2"),
            LayoutId::Layout3 => write!(f, "Layout 3"),
            LayoutId::Layout4 => write!(f, "Layout 4"),
        }
    }
}
impl LayoutId {
    pub const ALL: [LayoutId; 4] = [LayoutId::Layout1, LayoutId::Layout2, LayoutId::Layout3, LayoutId::Layout4];
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SerializableState {
    pub layouts: HashMap<LayoutId, SerializableDashboard>,
    pub last_active_layout: LayoutId,
    pub window_size: Option<(f32, f32)>,
    pub window_position: Option<(f32, f32)>,
}
impl SerializableState {
    pub fn from_parts(
        layouts: HashMap<LayoutId, SerializableDashboard>,
        last_active_layout: LayoutId,
        size: Option<Size>,
        position: Option<Point>,
    ) -> Self {
        SerializableState {
            layouts,
            last_active_layout,
            window_size: size.map(|s| (s.width, s.height)),
            window_position: position.map(|p| (p.x, p.y)),
        }
    }
}

use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn write_json_to_file(json: &str, file_path: &str) -> std::io::Result<()> {
    let path = Path::new(file_path);
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn read_layout_from_file(file_path: &str) -> Result<SerializableState, Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
   
    Ok(serde_json::from_str(&contents)?)
}