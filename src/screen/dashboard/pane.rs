use std::fmt;

use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

use crate::{
    charts::{
        candlestick::CandlestickChart, footprint::FootprintChart, heatmap::HeatmapChart, timeandsales::TimeAndSales
    }, 
    data_providers::{
        Exchange, TickMultiplier, Ticker, Timeframe
    }, 
    StreamType
};

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

    pub fn matches_stream(&self, stream_type: &StreamType) -> bool {
        self.stream.iter().any(|stream| stream == stream_type)
    }
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