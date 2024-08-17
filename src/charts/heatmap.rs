use std::{collections::{BTreeMap, HashMap, VecDeque}, rc::Rc, time::Instant};
use chrono::NaiveDateTime;
use iced::{
    alignment, mouse, widget::{button, canvas::{self, event::{self, Event}, stroke::Stroke, Canvas, Geometry, Path}}, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector
};
use iced::widget::{Column, Row, Container, Text};

use crate::data_providers::{Depth, Order, Trade};

use super::{Chart, CommonChartData, Message, chart_button, Interaction, AxisLabelYCanvas, AxisLabelXCanvas};

#[derive(Debug, Clone, Default)]
pub struct GroupedDepth {
    pub bids: Box<[Order]>,
    pub asks: Box<[Order]>,
}
pub struct GroupedTrade {
    pub is_sell: bool,
    pub price: f32,
    pub qty: f32,
}

#[derive(Default)]
struct QtyScale {
    max_trade_qty: f32,
    min_trade_qty: f32,
    max_aggr_volume: f32,
    max_depth_qty: f32,
}

pub struct HeatmapChart {
    chart: CommonChartData,
    data_points: Vec<(i64, (GroupedDepth, Box<[GroupedTrade]>))>,
    tick_size: f32,
    y_scaling: i32,
    size_filter: f32,
    qty_scales: QtyScale,
}

impl Chart for HeatmapChart {
    type DataPoint = Vec<(i64, (GroupedDepth, Box<[GroupedTrade]>))>;

    fn get_common_data(&self) -> &CommonChartData {
        &self.chart
    }
    fn get_common_data_mut(&mut self) -> &mut CommonChartData {
        &mut self.chart
    }
}

impl HeatmapChart {
    const MIN_SCALING: f32 = 1.0;
    const MAX_SCALING: f32 = 3.0;

    pub fn new(tick_size: f32) -> Self {
        HeatmapChart {
            chart: CommonChartData::default(),
            data_points: Vec::new(),
            tick_size,
            y_scaling: 100,
            size_filter: 0.0,
            qty_scales: QtyScale::default(),
        }
    }

    fn group_by_price(&self, orders: &[Order], is_bid: bool) -> Box<[Order]> {
        let mut grouped: HashMap<i64, f32> = HashMap::new();

        for &order in orders {
            let rounded_price = if is_bid {
                ((order.price * (1.0 / self.tick_size)).floor()) as i64
            } else {
                ((order.price * (1.0 / self.tick_size)).ceil()) as i64
            };
            *grouped.entry(rounded_price).or_insert(0.0) += order.qty;
        }

        grouped.into_iter().map(
            |(price, qty)| Order {
                price: price as f32 * self.tick_size, 
                qty 
            }
        ).collect()
    }

    pub fn set_size_filter(&mut self, size_filter: f32) {
        self.size_filter = size_filter;
    }
    pub fn get_size_filter(&self) -> f32 {
        self.size_filter
    }

    pub fn change_tick_size(&mut self, tick_size: f32) {
        self.tick_size = tick_size;

        self.data_points.clear();
    }

    pub fn insert_datapoint(&mut self, trades_buffer: &[Trade], depth_update: i64, depth: Rc<Depth>) {
        let aggregate_time = 100; // 100 ms
        let rounded_depth_update = (depth_update / aggregate_time) * aggregate_time;

        let grouped_depth = {
            let mut grouped_bids = self.group_by_price(&depth.bids, true);
            let mut grouped_asks = self.group_by_price(&depth.asks, false);

            grouped_bids.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
            grouped_asks.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());

            GroupedDepth {
                bids: grouped_bids,
                asks: grouped_asks,
            }
        };

        let grouped_trades = trades_buffer
            .iter()
            .map(|trade| GroupedTrade {
                is_sell: trade.is_sell,
                price: {
                    let grouped_price = if trade.is_sell {
                        (trade.price * (1.0 / self.tick_size)).floor()
                    } else {
                        (trade.price * (1.0 / self.tick_size)).ceil()
                    };
                    grouped_price as f32 * self.tick_size
                },
                qty: trade.qty,
            })
            .collect();
        
        self.data_points.push((rounded_depth_update, (grouped_depth, grouped_trades)));
    
        if self.data_points.len() > 2400 {
            self.data_points.drain(0..400);
        }
        
        self.render_start();
    }

    fn visible_data_iter(
        &self, 
        earliest: i64, latest: i64
    ) -> impl Iterator<Item = &(i64, (GroupedDepth, Box<[GroupedTrade]>))> {
        self.data_points.iter().filter(move |(time, _)| *time >= earliest && *time <= latest)
    }

    fn calculate_scales(&self) -> (i64, i64, f32, f32, QtyScale) {
        //let start = Instant::now();

        let timestamp_latest: &i64 = self.data_points.last().map(|(timestamp, _)| timestamp).unwrap_or(&0);

        let latest: i64 = *timestamp_latest - ((self.chart.translation.x - (self.chart.bounds.width/20.0)) * 60.0) as i64;
        let earliest: i64 = latest - (48000.0 / (self.chart.scaling / (self.chart.bounds.width/800.0))) as i64;
    
        let (mut highest, mut lowest) = (0.0f32, f32::MAX);

        let (mut min_trade_qty, mut max_trade_qty) = (f32::MAX, 0.0f32);
        let (mut max_aggr_volume, mut max_depth_qty) = (0.0f32, 0.0f32);

        let (autoscale, y_scaling) = (self.chart.autoscale, self.y_scaling as f32);
        let tick_size = self.tick_size;

        for (_, (depth, _)) in self.visible_data_iter(earliest, latest) {
            let mid_price = (
                depth.bids.last().map(|order| order.price).unwrap_or(0.0)
                + depth.asks.first().map(|order| order.price).unwrap_or(0.0)
            ) / 2.0;
    
            if autoscale {
                highest = highest.max(mid_price + (100.0 * tick_size));
                lowest = lowest.min(mid_price - (100.0 * tick_size));
            } else {
                highest = highest.max(mid_price + (y_scaling * tick_size));
                lowest = lowest.min(mid_price - (y_scaling * tick_size));
            }
        }

        for (_, (depth, trades)) in self.visible_data_iter(earliest, latest) {
            let (mut buy_volume, mut sell_volume) = (0.0, 0.0);

            for trade in trades.iter() {
                max_trade_qty = max_trade_qty.max(trade.qty);
                min_trade_qty = min_trade_qty.min(trade.qty);

                if trade.is_sell {
                    sell_volume += trade.qty;
                } else {
                    buy_volume += trade.qty;
                }
            }

            max_aggr_volume = max_aggr_volume.max(buy_volume).max(sell_volume);

            for order in depth.asks.iter().chain(depth.bids.iter()) {
                if order.price > highest || order.price < lowest {
                    continue;
                }
                max_depth_qty = max_depth_qty.max(order.qty);
            }
        }

        //log::info!("Heatmap scales calculation time: {:?}us", start.elapsed().as_micros());

        (
            latest, 
            earliest, 
            highest, 
            lowest, 
            QtyScale {
                max_trade_qty,
                min_trade_qty,
                max_aggr_volume,
                max_depth_qty
            }
        )
    }

    fn render_start(&mut self) {  
        let (
            latest, 
            earliest, 
            highest, 
            lowest, 
            visible_qty_scales
        ) = self.calculate_scales();

        if latest == 0 || highest == 0.0 || lowest == 0.0 {
            return;
        }

        self.qty_scales = visible_qty_scales;

        let chart_state = self.get_common_data_mut();

        if earliest != chart_state.x_min_time || latest != chart_state.x_max_time {         
            chart_state.x_min_time = earliest;
            chart_state.x_max_time = latest;

            chart_state.x_labels_cache.clear();
            chart_state.x_crosshair_cache.clear();
        };

        if lowest != chart_state.y_min_price || highest != chart_state.y_max_price {   
            chart_state.y_min_price = lowest;
            chart_state.y_max_price = highest;

            chart_state.y_labels_cache.clear();
            chart_state.y_crosshair_cache.clear();
        };
        
        chart_state.crosshair_cache.clear();     
        chart_state.main_cache.clear();   
    }

    pub fn update(&mut self, message: &Message) {
        match message {
            Message::Translated(translation) => {
                let chart = self.get_common_data_mut();

                if chart.autoscale {
                    chart.translation.x = translation.x;
                } else {
                    chart.translation = *translation;
                }
                chart.crosshair_position = Point::new(0.0, 0.0);

                self.render_start();
            },
            Message::Scaled(scaling, translation) => {
                let chart = self.get_common_data_mut();

                chart.scaling = *scaling;
                
                if let Some(translation) = translation {
                    if chart.autoscale {
                        chart.translation.x = translation.x;
                    } else {
                        chart.translation = *translation;
                    }
                }
                chart.crosshair_position = Point::new(0.0, 0.0);

                self.render_start();
            },
            Message::ChartBounds(bounds) => {
                self.chart.bounds = *bounds;
            },
            Message::AutoscaleToggle => {
                self.chart.autoscale = !self.chart.autoscale;

                if self.chart.autoscale {
                    self.chart.translation = Vector::default();

                    self.chart.scaling = 1.0;
                }
            },
            Message::CrosshairToggle => {
                self.chart.crosshair = !self.chart.crosshair;
            },
            Message::CrosshairMoved(position) => {
                let chart = self.get_common_data_mut();

                chart.crosshair_position = *position;
                if chart.crosshair {
                    chart.crosshair_cache.clear();
                    chart.y_crosshair_cache.clear();
                    chart.x_crosshair_cache.clear();
                }
            },
            Message::YScaling(delta, is_wheel_scroll) => {
                if self.chart.autoscale {
                    self.chart.autoscale = false;
                }

                let scaling_factor = if *is_wheel_scroll {
                    10.0
                } else {
                    2.0
                };

                if *delta < 1.0 {
                    if self.y_scaling < 200 {
                        self.y_scaling = (self.y_scaling + (delta * scaling_factor) as i32).min(200);
                    }
                } else {
                    if self.y_scaling > 20 {
                        self.y_scaling = (self.y_scaling - (delta * scaling_factor) as i32).max(20);
                    }
                }
            },
        }
    }

    pub fn view(&self) -> Element<Message> {
        let chart = Canvas::new(self)
            .width(Length::FillPortion(10))
            .height(Length::FillPortion(10));

        let chart_state = self.get_common_data();
        
        let axis_labels_x = Canvas::new(
            AxisLabelXCanvas { 
                labels_cache: &chart_state.x_labels_cache, 
                min: chart_state.x_min_time, 
                max: chart_state.x_max_time, 
                crosshair_cache: &chart_state.x_crosshair_cache, 
                crosshair_position: chart_state.crosshair_position, 
                crosshair: chart_state.crosshair,
                timeframe: None,
            })
            .width(Length::FillPortion(10))
            .height(Length::Fixed(26.0));

        let axis_labels_y = Canvas::new(
            AxisLabelYCanvas { 
                labels_cache: &chart_state.y_labels_cache, 
                y_croshair_cache: &chart_state.y_crosshair_cache, 
                min: chart_state.y_min_price,
                max: chart_state.y_max_price,
                crosshair_position: chart_state.crosshair_position, 
                crosshair: chart_state.crosshair,
            })
            .width(Length::Fixed(60.0))
            .height(Length::FillPortion(10));

        let autoscale_button = button(
            Text::new("A")
                .size(12)
                .align_x(alignment::Horizontal::Center)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(Message::AutoscaleToggle)
            .style(|_theme: &Theme, _status: iced::widget::button::Status| chart_button(_theme, _status, chart_state.autoscale));
        let crosshair_button = button(
            Text::new("+")
                .size(12)
                .align_x(alignment::Horizontal::Center)
            ) 
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(Message::CrosshairToggle)
            .style(|_theme: &Theme, _status: iced::widget::button::Status| chart_button(_theme, _status, chart_state.crosshair));
    
        let chart_controls = Container::new(
            Row::new()
                .push(autoscale_button)
                .push(crosshair_button).spacing(2)
            ).padding([0, 2])
            .width(Length::Fixed(60.0))
            .height(Length::Fixed(26.0));

        let chart_and_y_labels = Row::new()
            .push(chart)
            .push(axis_labels_y);
    
        let bottom_row = Row::new()
            .push(axis_labels_x)
            .push(chart_controls);
    
        let content = Column::new()
            .push(chart_and_y_labels)
            .push(bottom_row)
            .spacing(0)
            .padding(5);
    
        content.into()
    }
}

impl canvas::Program<Message> for HeatmapChart {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        let chart_state = self.get_common_data();

        if bounds != chart_state.bounds {
            return (event::Status::Ignored, Some(Message::ChartBounds(bounds)));
        } 
    
        if let Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
            *interaction = Interaction::None;
        }

        let Some(cursor_position) = cursor.position_in(bounds) else {
            return (event::Status::Ignored, 
                if chart_state.crosshair {
                    Some(Message::CrosshairMoved(Point::new(0.0, 0.0)))
                } else {
                    None
                }
                );
        };

        match event {
            Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::ButtonPressed(button) => {
                    let message = match button {
                        mouse::Button::Left => {
                            *interaction = Interaction::Panning {
                                translation: chart_state.translation,
                                start: cursor_position,
                            };
                            None
                        }
                        _ => None,
                    };

                    (event::Status::Captured, message)
                }
                mouse::Event::CursorMoved { .. } => {
                    let message = match *interaction {
                        Interaction::Panning { translation, start } => {
                            Some(
                                Message::Translated(
                                    translation + (cursor_position - start) * (1.0 / chart_state.scaling),
                                )
                            )
                        }
                        Interaction::None => 
                            if chart_state.crosshair && cursor.is_over(bounds) {
                                Some(Message::CrosshairMoved(cursor_position))
                            } else {
                                None
                            },
                        _ => None,
                    };

                    let event_status = match interaction {
                        Interaction::None => event::Status::Ignored,
                        _ => event::Status::Captured,
                    };

                    (event_status, message)
                }
                mouse::Event::WheelScrolled { delta } => match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {                        
                        if y < 0.0 && chart_state.scaling > Self::MIN_SCALING
                            || y > 0.0 && chart_state.scaling < Self::MAX_SCALING 
                        {
                            let scaling = (chart_state.scaling * (1.0 + y / 30.0))
                                .clamp(Self::MIN_SCALING, Self::MAX_SCALING);

                            (event::Status::Captured, Some(Message::Scaled(scaling, None)))
                        } else {
                            (event::Status::Captured, None)
                        }
                    }
                }
                _ => (event::Status::Ignored, None),
            },
            _ => (event::Status::Ignored, None),
        }
    }
    
    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {    
        let chart = self.get_common_data();

        let (latest, earliest) = (chart.x_max_time, chart.x_min_time);    
        let (lowest, highest) = (chart.y_min_price, chart.y_max_price);

        let y_range: f32 = highest - lowest;
        
        let volume_area_height: f32 = bounds.height / 8.0; 
        let heatmap_area_height: f32 = bounds.height - volume_area_height;

        let depth_area_width: f32 = bounds.width / 20.0;

        let heatmap = chart.main_cache.draw(renderer, bounds.size(), |frame| {
            //let start = Instant::now();
            let mut bar_height: f32 = 1.0;

            let max_aggr_volume = self.qty_scales.max_aggr_volume;
            let max_depth_qty = self.qty_scales.max_depth_qty;
            let (min_trade_qty, max_trade_qty) = (self.qty_scales.min_trade_qty, self.qty_scales.max_trade_qty);

            // draw: current depth as bars on the right side
            if let Some((latest_timestamp, (grouped_depth, _))) = self.data_points.last() {
                let x_position = ((latest_timestamp - earliest) as f32 / (latest - earliest) as f32) * bounds.width;

                if x_position.is_nan() {
                    return;
                }

                let latest_bids: Vec<(f32, f32)> = grouped_depth.bids.iter()
                    .map(|order| (order.price, order.qty))
                    .filter(|&(price, _)| price >= lowest)
                    .collect();

                let latest_asks: Vec<(f32, f32)> = grouped_depth.asks.iter()
                    .map(|order| (order.price, order.qty))
                    .filter(|&(price, _)| price <= highest)
                    .collect();

                let highest_ask_visible = latest_asks.last()
                    .map(|(price, _)| *price)
                    .unwrap_or(highest);
                let highest_ask_y_pos = heatmap_area_height - ((highest_ask_visible - lowest) / y_range * heatmap_area_height);

                let lowest_bid_visible = latest_bids.first()
                    .map(|(price, _)| *price)
                    .unwrap_or(lowest); 
                let lowest_bid_y_pos = heatmap_area_height - ((lowest_bid_visible - lowest) / y_range * heatmap_area_height);
                
                bar_height = (((lowest_bid_y_pos - highest_ask_y_pos) / (y_range / self.tick_size) as f32).floor()).max(1.0);

                let mut max_qty = latest_bids.iter()
                    .map(|(_, qty)| qty)
                    .chain(latest_asks.iter().map(|(_, qty)| qty))
                    .fold(f32::MIN, |price: f32, qty: &f32| f32::max(price, *qty));
                max_qty = (max_qty / 5.0).ceil() * 5.0;

                for (price, qty) in &latest_bids {     
                    let y_position = heatmap_area_height - ((price - lowest) / y_range * heatmap_area_height);
                
                    let bar_width = (qty / max_qty) * depth_area_width;

                    frame.fill_rectangle(
                        Point::new(x_position, y_position - (bar_height/2.0)), 
                        Size::new(bar_width, bar_height), 
                        Color::from_rgba8(0, 144, 144, 0.5)
                    );
                }
                
                for (price, qty) in &latest_asks {
                    let y_position = heatmap_area_height - ((price - lowest) / y_range * heatmap_area_height);
                
                    let bar_width = (qty / max_qty) * depth_area_width;

                    frame.fill_rectangle(
                        Point::new(x_position, y_position - (bar_height/2.0)), 
                        Size::new(bar_width, bar_height), 
                        Color::from_rgba8(192, 0, 192, 0.5)
                    );
                }
                
                // the white bar to seperate the heatmap area
                frame.fill_rectangle(
                    Point::new(x_position, 0.0), 
                    Size::new(1.0, bounds.height), 
                    Color::from_rgba8(100, 100, 100, 0.2)
                );

                // max bid/ask quantity text
                let text_size = 9.0;
                let text_content = format!("{max_qty:.0}");
                let text_position = Point::new(x_position + depth_area_width, 0.0);
                frame.fill_text(canvas::Text {
                    content: text_content,
                    position: text_position,
                    size: iced::Pixels(text_size),
                    color: Color::from_rgba8(121, 121, 121, 1.0),
                    ..canvas::Text::default()
                });

                // max aggregated volume text
                let text_content = format!("{max_aggr_volume:.2}");
                if x_position > bounds.width {      
                    let text_width = (text_content.len() as f32 * text_size) / 1.5;

                    let text_position = Point::new(bounds.width - text_width, bounds.height - (volume_area_height - bar_height));
                    
                    frame.fill_text(canvas::Text {
                        content: text_content,
                        position: text_position,
                        size: iced::Pixels(text_size),
                        color: Color::from_rgba8(121, 121, 121, 1.0),
                        ..canvas::Text::default()
                    });

                } else {
                    let text_position = Point::new(x_position + 5.0, bounds.height - (volume_area_height - bar_height));

                    frame.fill_text(canvas::Text {
                        content: text_content,
                        position: text_position,
                        size: iced::Pixels(text_size),
                        color: Color::from_rgba8(121, 121, 121, 1.0),
                        ..canvas::Text::default()
                    });
                }
            };

            // draw: depth heatmap and trades
            let mut prev_bid_price: Option<f32> = None;
            let mut prev_bid_qty: Option<f32> = None;
            let mut prev_ask_price: Option<f32> = None;
            let mut prev_ask_qty: Option<f32> = None;

            let mut prev_x_position: Option<f32> = None;

            for (time, (depth, trades)) in self.visible_data_iter(earliest, latest) {
                let x_position = ((time - earliest) as f32 / (latest - earliest) as f32) * bounds.width;

                if x_position.is_nan() {
                    continue;
                }

                for order in depth.bids.iter() {
                    if order.price >= lowest {
                        if let (Some(prev_price), Some(prev_qty), Some(prev_x)) = (prev_bid_price, prev_bid_qty, prev_x_position) {
                            let y_position = heatmap_area_height - ((order.price - lowest) / y_range * heatmap_area_height);
                            let color_alpha = (order.qty / max_depth_qty).min(1.0);

                            if prev_price != order.price || prev_qty != order.qty {
                                frame.fill_rectangle(
                                    Point::new(prev_x, y_position - (bar_height/2.0)),
                                    Size::new(x_position - prev_x, bar_height),
                                    Color::from_rgba8(0, 144, 144, color_alpha)
                                );
                            }
                        }
                        prev_bid_price = Some(order.price);
                        prev_bid_qty = Some(order.qty);
                    }
                }

                for order in depth.asks.iter() {
                    if order.price <= highest {
                        if let (Some(prev_price), Some(prev_qty), Some(prev_x)) = (prev_ask_price, prev_ask_qty, prev_x_position) {
                            let y_position = heatmap_area_height - ((order.price - lowest) / y_range * heatmap_area_height);
                            let color_alpha = (order.qty / max_depth_qty).min(1.0);

                            if prev_price != order.price || prev_qty != order.qty {
                                frame.fill_rectangle(
                                    Point::new(prev_x, y_position - (bar_height/2.0)), 
                                    Size::new(x_position - prev_x, bar_height), 
                                    Color::from_rgba8(192, 0, 192, color_alpha)
                                );
                            }
                        }
                        prev_ask_price = Some(order.price);
                        prev_ask_qty = Some(order.qty);
                    }
                }

                prev_x_position = Some(x_position);

                let mut buy_volume: f32 = 0.0;
                let mut sell_volume: f32 = 0.0;

                for trade in trades.iter() {
                    if trade.is_sell {
                        sell_volume += trade.qty;
                    } else {
                        buy_volume += trade.qty;
                    }

                    if trade.price < lowest || trade.price > highest {
                        continue;
                    }

                    if trade.qty *  trade.price > self.size_filter {
                        let x_position = (((time - 100) - earliest) as f32 / (latest - earliest) as f32) * bounds.width;
                        let y_position = heatmap_area_height - ((trade.price - lowest) / y_range * heatmap_area_height);

                        let color = if trade.is_sell {
                            Color::from_rgba8(192, 80, 77, 1.0)
                        } else {
                            Color::from_rgba8(81, 205, 160, 1.0)
                        };

                        let radius: f32 = match max_trade_qty == min_trade_qty {
                            true => 1.0,
                            false => 1.0 + (trade.qty - min_trade_qty) * (35.0 - 1.0) / (max_trade_qty - min_trade_qty),
                        };

                        frame.fill(
                            &Path::circle(Point::new(x_position, y_position), radius), 
                            color
                        );
                    }
                }

                if max_aggr_volume > 0.0 {
                    let buy_bar_height = (buy_volume / max_aggr_volume) * (volume_area_height - bar_height);
                    frame.fill_rectangle(
                        Point::new(x_position as f32 + 2.0, bounds.height - buy_bar_height), 
                        Size::new(1.0, buy_bar_height), 
                        Color::from_rgb8(81, 205, 160)
                    );

                    let sell_bar_height = (sell_volume / max_aggr_volume) * (volume_area_height - bar_height);
                    frame.fill_rectangle(
                        Point::new(x_position as f32, bounds.height - sell_bar_height), 
                        Size::new(1.0, sell_bar_height), 
                        Color::from_rgb8(192, 80, 77)
                    );
                }
            };

            //log::info!("Heatmap draw time: {:?}us", start.elapsed().as_micros());
        });

        if chart.crosshair {
            let crosshair = chart.crosshair_cache.draw(renderer, bounds.size(), |frame| {
                if let Some(cursor_position) = cursor.position_in(bounds) {
                    let line = Path::line(
                        Point::new(0.0, cursor_position.y), 
                        Point::new(bounds.width, cursor_position.y)
                    );
                    frame.stroke(&line, Stroke::default().with_color(Color::from_rgba8(200, 200, 200, 0.6)).with_width(1.0));

                    let crosshair_ratio = cursor_position.x as f64 / bounds.width as f64;
                    let crosshair_millis = (earliest as f64 + crosshair_ratio * (latest as f64 - earliest as f64)).round() / 100.0 * 100.0;
                    let crosshair_time = NaiveDateTime::from_timestamp((crosshair_millis / 1000.0).floor() as i64, ((crosshair_millis % 1000.0) * 1_000_000.0).round() as u32);

                    let crosshair_timestamp = crosshair_time.timestamp_millis();

                    let snap_ratio = (crosshair_timestamp as f64 - earliest as f64) / ((latest as f64) - (earliest as f64));
                    let snap_x = snap_ratio * bounds.width as f64;

                    let line = Path::line(
                        Point::new(snap_x as f32, 0.0), 
                        Point::new(snap_x as f32, bounds.height)
                    );
                    frame.stroke(&line, Stroke::default().with_color(Color::from_rgba8(200, 200, 200, 0.6)).with_width(1.0));
                }
            });

            vec![crosshair, heatmap]
        }   else {
            vec![heatmap]
        }
    }

    fn mouse_interaction(
        &self,
        interaction: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match interaction {
            Interaction::Panning { .. } => mouse::Interaction::Grabbing,
            Interaction::Zoomin { .. } => mouse::Interaction::ZoomIn,
            Interaction::None if cursor.is_over(bounds) => {
                if self.chart.crosshair {
                    mouse::Interaction::Crosshair
                } else {
                    mouse::Interaction::default()
                }
            }
            Interaction::None => { mouse::Interaction::default() }
        }
    }
}