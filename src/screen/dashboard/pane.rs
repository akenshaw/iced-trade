use std::fmt;

use iced::{alignment, widget::{button, checkbox, container, pane_grid, pick_list, row, scrollable, text, tooltip, Column, Container, Row, Slider, Text}, Alignment, Element, Length, Renderer, Theme};
use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

use super::{Message as DashboardMessage};

use crate::{
    charts::{
        self, candlestick::CandlestickChart, footprint::FootprintChart, heatmap::HeatmapChart, timeandsales::TimeAndSales
    }, data_providers::{
        Exchange, TickMultiplier, Ticker, Timeframe
    }, modal, style::{self, Icon, ICON_FONT}, StreamType
};

#[derive(Debug, Clone)]
pub enum Message {
    PaneClicked(pane_grid::Pane),
    PaneResized(pane_grid::ResizeEvent),
    PaneDragged(pane_grid::DragEvent),
    ClosePane(pane_grid::Pane),
    SplitPane(pane_grid::Axis, pane_grid::Pane),
    MaximizePane(pane_grid::Pane),
    Restore,
    TicksizeSelected(TickMultiplier, Uuid),
    TimeframeSelected(Timeframe, Uuid),
    TickerSelected(Ticker, Uuid),
    ExchangeSelected(Exchange, Uuid),
    ShowModal(Uuid),
    HideModal(Uuid),
    PaneContentSelected(String, Uuid, Vec<StreamType>),
    ReplacePane(pane_grid::Pane),
    ChartUserUpdate(charts::Message, Uuid),
    SliderChanged(Uuid, f32),
}

#[derive(Debug)]
pub struct PaneState {
    pub id: Uuid,
    pub show_modal: bool,
    pub stream: Vec<StreamType>,
    pub content: PaneContent,
    pub settings: PaneSettings,
}

impl PaneState {
    pub fn new(id: Uuid, stream: Vec<StreamType>, settings: PaneSettings) -> Self {
        Self {
            id,
            show_modal: false,
            stream,
            content: PaneContent::Starter,
            settings,
        }
    }

    pub fn from_config(content: PaneContent, stream: Vec<StreamType>, settings: PaneSettings) -> Self {
        Self {
            id: Uuid::new_v4(),
            show_modal: false,
            stream,
            content,
            settings,
        }
    }

    pub fn view<'a>(
        &'a self,
        id: pane_grid::Pane,
        panes: usize,
        is_focused: bool,
        maximized: bool,
    ) -> iced::widget::pane_grid::Content<'a, Message, Theme, Renderer> {
        let stream_info = self.stream.iter().find_map(|stream: &StreamType| {
            match stream {
                StreamType::Kline { exchange, ticker, timeframe } => {
                    Some(
                        Some((exchange, format!("{} {}", ticker, timeframe)))
                    )
                }
                _ => None,
            }
        }).or_else(|| {
            self.stream.iter().find_map(|stream: &StreamType| {
                match stream {
                    StreamType::DepthAndTrades { exchange, ticker } => {
                        Some(
                            Some((exchange, ticker.to_string()))
                        )
                    }
                    _ => None,
                }
            })
        }).unwrap_or(None);

        let mut stream_info_element: Row<Message> = Row::new();

        if let Some((exchange, info)) = stream_info {
            stream_info_element = Row::new()
                .spacing(3)
                .push(
                    match exchange {
                        Exchange::BinanceFutures => text(char::from(Icon::BinanceLogo).to_string()).font(ICON_FONT),
                        Exchange::BybitLinear => text(char::from(Icon::BybitLogo).to_string()).font(ICON_FONT),
                    }
                )
                .push(Text::new(info));
        }
        
        let mut content: pane_grid::Content<'_, Message, _, Renderer> = 
            pane_grid::Content::new({
                match self.content {
                    PaneContent::Starter => view_starter(&self.id, &self.settings),

                    _ => {
                        Column::new()
                            .push(Text::new("Loading..."))
                            .into()
                    }
                }
            })
            .style(
                if is_focused {
                    style::pane_focused
                } else {
                    style::pane_active
                }
            );

        let title_bar = pane_grid::TitleBar::new(stream_info_element)
            .controls(view_controls(
                id,
                self.id,
                &self.content,
                panes,
                maximized,
                &self.settings,
            ))
            .padding(4)
            .style(
                if is_focused {
                    style::title_bar_focused
                } else {
                    style::title_bar_active
                }
            );
        content = content.title_bar(title_bar);

        content
    }

    pub fn matches_stream(&self, stream_type: &StreamType) -> bool {
        self.stream.iter().any(|stream| stream == stream_type)
    }
}

trait ChartView {
    fn view(&self, id: &PaneState) -> Element<Message>;
}

impl ChartView for HeatmapChart {
    fn view(&self, pane: &PaneState) -> Element<Message> {
        let pane_id = pane.id;

        let underlay = self.view().map(move |message| Message::ChartUserUpdate(message, pane_id));

        if pane.show_modal {
            let size_filter = &self.get_size_filter();

            let signup: Container<Message, Theme, _> = container(
                Column::new()
                    .spacing(10)
                    .align_x(Alignment::Center)
                    .push(
                        Text::new("Heatmap > Settings")
                            .size(16)
                    )
                    .push(
                        Column::new()
                            .align_x(Alignment::Center)
                            .push(Text::new("Size Filtering"))
                            .push(
                                Slider::new(0.0..=50000.0, *size_filter, move |value| Message::SliderChanged(pane_id, value))
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
                                .on_press(Message::HideModal(pane_id))
                            )
                    )
            )
            .width(Length::Shrink)
            .padding(20)
            .max_width(500)
            .style(style::chart_modal);

            return modal(underlay, signup, Message::HideModal(pane_id));
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
        let pane_id = pane.id;

        let underlay = self.view();

        if pane.show_modal {
            let size_filter = &self.get_size_filter();

            let signup = container(
                Column::new()
                    .spacing(10)
                    .align_x(Alignment::Center)
                    .push(
                        Text::new("Time&Sales > Settings")
                            .size(16)
                    )
                    .push(
                        Column::new()
                            .align_x(Alignment::Center)
                            .push(Text::new("Size Filtering"))
                            .push(
                                Slider::new(0.0..=50000.0, *size_filter, move |value| Message::SliderChanged(pane_id, value))
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
                                .on_press(Message::HideModal(pane_id))
                            )
                    )
            )
            .width(Length::Shrink)
            .padding(20)
            .max_width(500)
            .style(style::chart_modal);

            return modal(underlay, signup, Message::HideModal(pane_id));
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
        (Icon::ResizeFull, Message::MaximizePane(pane))
    };

    match pane_type {
        PaneContent::Heatmap(_) => {
            let ticksize_picker = pick_list(
                [TickMultiplier(1), TickMultiplier(2), TickMultiplier(5), TickMultiplier(10), TickMultiplier(25), TickMultiplier(50)],
                settings.tick_multiply, 
                move |tick_multiply| Message::TicksizeSelected(tick_multiply, pane_id)
            ).placeholder("Ticksize multiplier...").text_size(11).width(iced::Pixels(80.0));

            let ticksize_tooltip = tooltip(
                ticksize_picker
                    .style(style::picklist_primary)
                    .menu_style(style::picklist_menu_primary),
                    "Ticksize multiplier",
                    tooltip::Position::FollowCursor
                )
                .style(style::tooltip);
    
            row = row.push(ticksize_tooltip);
        },
        PaneContent::TimeAndSales(_) => {
        },
        PaneContent::Footprint(_) => {
            let timeframe_picker = pick_list(
                &Timeframe::ALL[..],
                settings.selected_timeframe,
                move |timeframe| Message::TimeframeSelected(timeframe, pane_id),
            ).placeholder("Choose a timeframe...").text_size(11).width(iced::Pixels(80.0));
    
            let tf_tooltip = tooltip(
                timeframe_picker
                    .style(style::picklist_primary)
                    .menu_style(style::picklist_menu_primary),
                    "Timeframe",
                    tooltip::Position::FollowCursor
                )
                .style(style::tooltip);
    
            row = row.push(tf_tooltip);

            let ticksize_picker = pick_list(
                [TickMultiplier(1), TickMultiplier(2), TickMultiplier(5), TickMultiplier(10), TickMultiplier(25), TickMultiplier(50), TickMultiplier(100), TickMultiplier(200)],
                settings.tick_multiply, 
                move |tick_multiply| Message::TicksizeSelected(tick_multiply, pane_id)
            ).placeholder("Ticksize multiplier...").text_size(11).width(iced::Pixels(80.0));
            
            let ticksize_tooltip = tooltip(
                ticksize_picker
                    .style(style::picklist_primary)
                    .menu_style(style::picklist_menu_primary),
                    "Ticksize multiplier",
                    tooltip::Position::FollowCursor
                )
                .style(style::tooltip);
    
            row = row.push(ticksize_tooltip);
        },
        PaneContent::Candlestick(_) => {
            let timeframe_picker = pick_list(
                &Timeframe::ALL[..],
                settings.selected_timeframe,
                move |timeframe| Message::TimeframeSelected(timeframe, pane_id),
            ).placeholder("Choose a timeframe...").text_size(11).width(iced::Pixels(80.0));
    
            let tooltip = tooltip(
                timeframe_picker
                    .style(style::picklist_primary)
                    .menu_style(style::picklist_menu_primary),
                    "Timeframe", 
                    tooltip::Position::FollowCursor
                )
                .style(style::tooltip);
    
            row = row.push(tooltip);
        },
        PaneContent::Starter => {
        },
    }

    let mut buttons = vec![
        (container(text(char::from(Icon::Cog).to_string()).font(ICON_FONT).size(14)).width(25).center_x(iced::Pixels(25.0)), Message::ShowModal(pane_id)),
        (container(text(char::from(icon).to_string()).font(ICON_FONT).size(14)).width(25).center_x(iced::Pixels(25.0)), message),
    ];

    if total_panes > 1 {
        buttons.push((container(text(char::from(Icon::Close).to_string()).font(ICON_FONT).size(14)).width(25).center_x(iced::Pixels(25.0)), Message::ClosePane(pane)));
    }

    for (content, message) in buttons {        
        row = row.push(
            button(content)
                .style(style::button_primary)
                .padding(3)
                .on_press(message),
        );
    } 

    row.into()
}

fn view_starter<'a>(
    pane_id: &'a Uuid,
    pane_settings: &'a PaneSettings,
) -> Element<'a, Message> {
    let content_names = ["Heatmap chart", "Footprint chart", "Candlestick chart", "Time&Sales"];
    
    let content_selector = content_names.iter().fold(
        Column::new()
            .spacing(6)
            .align_x(Alignment::Center), |column, &label| {
                let mut btn = button(label).width(Length::Fill);
                if let (Some(exchange), Some(ticker)) = (pane_settings.selected_exchange, pane_settings.selected_ticker) {
                    let timeframe = pane_settings.selected_timeframe.unwrap_or_else(
                        || { log::error!("No timeframe found"); Timeframe::M1 }
                    );

                    let pane_stream: Vec<StreamType> = match label {
                        "Heatmap chart" | "Time&Sales" => vec![
                            StreamType::DepthAndTrades { exchange, ticker }
                        ],
                        "Footprint chart" => vec![
                            StreamType::DepthAndTrades { exchange, ticker }, 
                            StreamType::Kline { exchange, ticker, timeframe }
                        ],
                        "Candlestick chart" => vec![
                            StreamType::Kline { exchange, ticker, timeframe }
                        ],
                        _ => vec![]
                    };
                
                    btn = btn.on_press(
                        Message::PaneContentSelected(
                            label.to_string(),
                            *pane_id,
                            pane_stream
                        )
                    );
                }
                column.push(btn)
            }
    );

    let symbol_selector = pick_list(
        &Ticker::ALL[..],
        pane_settings.selected_ticker,
        move |ticker| Message::TickerSelected(ticker, *pane_id),
    ).placeholder("ticker...").text_size(13).width(Length::Fill);

    let exchange_selector = pick_list(
        &Exchange::ALL[..],
        pane_settings.selected_exchange,
        move |exchange| Message::ExchangeSelected(exchange, *pane_id),
    ).placeholder("exchange...").text_size(13).width(Length::Fill);

    let picklists = Row::new()
        .spacing(6)
        .align_y(Alignment::Center)
        .push(exchange_selector.style(style::picklist_primary).menu_style(style::picklist_menu_primary))
        .push(symbol_selector.style(style::picklist_primary).menu_style(style::picklist_menu_primary));

    let column = Column::new()
        .padding(10)
        .spacing(10)
        .align_x(Alignment::Center)
        .push(picklists)
        .push(content_selector);
        
    let container = Container::new(
        Column::new()
            .spacing(10)
            .padding(20)
            .align_x(Alignment::Center)
            .max_width(300)
            .push(
                Text::new("Initialize the pane").size(16)
            )
            .push(scrollable(column))
        ).align_x(alignment::Horizontal::Center);
    
    container.into()
}

pub enum PaneContent {
    Heatmap(HeatmapChart),
    Footprint(FootprintChart),
    Candlestick(CandlestickChart),
    TimeAndSales(TimeAndSales),
    Starter,
}

impl fmt::Debug for PaneContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaneContent::Heatmap(_) => write!(f, "Heatmap"),
            PaneContent::Footprint(_) => write!(f, "Footprint"),
            PaneContent::Candlestick(_) => write!(f, "Candlestick"),
            PaneContent::TimeAndSales(_) => write!(f, "TimeAndSales"),
            PaneContent::Starter => write!(f, "Starter"),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct PaneSettings {
    pub min_tick_size: Option<f32>,
    pub trade_size_filter: Option<f32>,
    pub tick_multiply: Option<TickMultiplier>,
    pub selected_ticker: Option<Ticker>,
    pub selected_exchange: Option<Exchange>,
    pub selected_timeframe: Option<Timeframe>,
}
impl Default for PaneSettings {
    fn default() -> Self {
        Self {
            min_tick_size: None,
            trade_size_filter: Some(0.0),
            tick_multiply: Some(TickMultiplier(10)),
            selected_ticker: None,
            selected_exchange: None,
            selected_timeframe: Some(Timeframe::M1),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum SerializablePane {
    Split {
        axis: Axis,
        ratio: f32,
        a: Box<SerializablePane>,
        b: Box<SerializablePane>,
    },
    Starter,
    HeatmapChart {
        stream_type: Vec<StreamType>,
        settings: PaneSettings,
    },
    FootprintChart {
        stream_type: Vec<StreamType>,
        settings: PaneSettings,
    },
    CandlestickChart {
        stream_type: Vec<StreamType>,
        settings: PaneSettings,
    },
    TimeAndSales {
        stream_type: Vec<StreamType>,
        settings: PaneSettings,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl From<&PaneState> for SerializablePane {
    fn from(pane: &PaneState) -> Self {
        let pane_stream = pane.stream.clone();

        match pane.content {
            PaneContent::Starter => SerializablePane::Starter,
            PaneContent::Heatmap(_) => SerializablePane::HeatmapChart {
                stream_type: pane_stream,
                settings: pane.settings,
            },
            PaneContent::Footprint(_) => SerializablePane::FootprintChart {
                stream_type: pane_stream,
                settings: pane.settings,
            },
            PaneContent::Candlestick(_) => SerializablePane::CandlestickChart {
                stream_type: pane_stream,
                settings: pane.settings,
            },
            PaneContent::TimeAndSales(_) => SerializablePane::TimeAndSales {
                stream_type: pane_stream,
                settings: pane.settings,
            }
        }
    }
}