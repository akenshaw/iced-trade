pub mod pane;

pub use pane::{Uuid, PaneState, PaneContent, PaneSettings};

use crate::{
    charts::Message, 
    data_providers::{
        Depth, Kline, TickMultiplier, Ticker, Timeframe, Trade
    }, 
    StreamType
};

use std::{collections::HashMap, rc::Rc};
use iced::widget::pane_grid::{self, Configuration};

pub struct Dashboard {
    pub panes: pane_grid::State<PaneState>,
    pub show_layout_modal: bool,
    pub focus: Option<pane_grid::Pane>,
    pub first_pane: pane_grid::Pane,
    pub pane_lock: bool,
    pub pane_state_cache: HashMap<Uuid, (Option<Ticker>, Option<Timeframe>, Option<f32>)>,
    pub last_axis_split: Option<pane_grid::Axis>,
}
impl Dashboard {
    pub fn empty(pane_config: Configuration<PaneState>) -> Self {
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

    pub fn update_chart_state(&mut self, pane_id: Uuid, message: Message) -> Result<(), &str> {
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

    pub fn get_pane_stream_mut(&mut self, pane_id: Uuid) -> Result<&mut Vec<StreamType>, &str> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                return Ok(&mut pane_state.stream);
            }
        }
        Err("No pane found")
    }

    pub fn get_pane_settings_mut(&mut self, pane_id: Uuid) -> Result<&mut PaneSettings, &str> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
                return Ok(&mut pane_state.settings);
            }
        }
        Err("No pane found")
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

    pub fn pane_change_ticksize(&mut self, pane_id: Uuid, new_tick_multiply: TickMultiplier) -> Result<(), &str> {
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
                        _ => {
                            return Err("No footprint chart found");
                        }
                    }
                } else {
                    return Err("No min tick size found");
                }
            }
        }
        Err("No pane found")
    }
    
    pub fn pane_set_size_filter(&mut self, pane_id: Uuid, new_size_filter: f32) -> Result<(), &str> {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.id == pane_id {
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
                        return Err("No footprint chart found");
                    }
                }
            }
        }
        Err("No pane found")
    }

    pub fn update_depth_and_trades(&mut self, stream_type: StreamType, depth_update_t: i64, depth: Depth, trades_buffer: Vec<Trade>) {
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

    pub fn insert_klines_vec(&mut self, stream_type: &StreamType, klines: &Vec<Kline>) {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.matches_stream(&stream_type) {
                match &mut pane_state.content {
                    PaneContent::Candlestick(chart) => chart.insert_klines(klines.to_vec()),
                    PaneContent::Footprint(chart) => chart.insert_klines(klines.to_vec()),
                    _ => {}
                }
            }
        }
    }

    pub fn update_latest_klines(&mut self, stream_type: &StreamType, kline: &Kline) {
        for (_, pane_state) in self.panes.iter_mut() {
            if pane_state.matches_stream(&stream_type) {
                match &mut pane_state.content {
                    PaneContent::Candlestick(chart) => chart.update_latest_kline(kline),
                    PaneContent::Footprint(chart) => chart.update_latest_kline(kline),
                    _ => {}
                }
            }
        }
    }
}