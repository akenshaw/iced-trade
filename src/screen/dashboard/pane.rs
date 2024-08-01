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

#[derive(Debug, Clone, Copy)]
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