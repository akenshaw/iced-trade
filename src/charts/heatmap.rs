use std::collections::{BTreeMap, HashMap};
use chrono::NaiveDateTime;
use iced::{
    alignment, color, mouse, widget::{button, canvas::{self, event::{self, Event}, stroke::Stroke, Cache, Canvas, Geometry, Path}}, window, Border, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector
};
use iced::widget::{Column, Row, Container, Text};
use crate::data_providers::binance::market_data::{LocalDepthCache, Trade};

use super::{Chart, CommonChartData, Message, chart_button, calculate_price_step, Interaction, AxisLabelYCanvas};

pub struct HeatmapChart {
    chart: CommonChartData,
    data_points: BTreeMap<i64, (LocalDepthCache, Box<[Trade]>)>,
    tick_size: f32,
    y_scaling: f32,
    size_filter: f32,
}

impl Chart for HeatmapChart {
    type DataPoint = BTreeMap<i64, (LocalDepthCache, Box<[Trade]>)>;

    fn get_common_data(&self) -> &CommonChartData {
        &self.chart
    }
    fn get_common_data_mut(&mut self) -> &mut CommonChartData {
        &mut self.chart
    }
}

impl HeatmapChart {
    const MIN_SCALING: f32 = 0.6;
    const MAX_SCALING: f32 = 3.6;

    pub fn new() -> Self {
        HeatmapChart {
            chart: CommonChartData::default(),
            data_points: BTreeMap::new(),
            tick_size: 0.0,
            y_scaling: 0.0001,
            size_filter: 0.0,
        }
    }

    pub fn set_size_filter(&mut self, size_filter: f32) {
        self.size_filter = size_filter;
    }

    pub fn get_raw_trades(&mut self) -> Vec<Trade> {
        let mut trades_source = vec![];

        for (_, trades) in self.data_points.values() {
            trades_source.extend(trades.iter().cloned());
        }

        trades_source
    }

    pub fn insert_datapoint(&mut self, trades_buffer: Vec<Trade>, depth_update: i64, depth: LocalDepthCache) {
        let aggregate_time = 100; // 100 ms
        let rounded_depth_update = (depth_update / aggregate_time) * aggregate_time;
        
        self.data_points.entry(rounded_depth_update).or_insert((depth, trades_buffer.into_boxed_slice()));
        
        if self.data_points.len() > 3600 {
            while let Some((&key_to_remove, _)) = self.data_points.iter().next() {
                self.data_points.remove(&key_to_remove);
                if self.data_points.len() <= 3000 {
                    break;
                }
            }
        }     

        self.render_start();
    }

    pub fn render_start(&mut self) {  
        let (latest, earliest, highest, lowest) = self.calculate_range();

        let chart = self.get_common_data_mut();

        if earliest != chart.x_min_time || latest != chart.x_max_time {         
            chart.x_min_time = earliest;
            chart.x_max_time = latest;

            chart.x_labels_cache.clear();
            chart.x_crosshair_cache.clear();
        };

        if lowest != chart.y_min_price || highest != chart.y_max_price {   
            chart.y_min_price = lowest;
            chart.y_max_price = highest;

            chart.y_labels_cache.clear();
            chart.y_crosshair_cache.clear();
        };
        
        chart.crosshair_cache.clear();     
        chart.main_cache.clear();   
    }

    fn calculate_range(&self) -> (i64, i64, f32, f32) {
        let timestamp_latest: &i64 = self.data_points.keys().last().unwrap_or(&0);

        let latest: i64 = *timestamp_latest - (self.chart.translation.x*80.0) as i64;
        let earliest: i64 = latest - (64000.0 / (self.chart.scaling / (self.chart.bounds.width/800.0))) as i64;
    
        let mut max_ask_price = f32::MIN;
        let mut min_bid_price = f32::MAX;

        for (_, (depth, _)) in self.data_points.range(earliest..=latest) {
            if !depth.asks.is_empty() && !depth.bids.is_empty() {        
                let ask_price: f32 = depth.asks[std::cmp::min(20, depth.asks.len() - 1)].price;
                let bid_price: f32 = depth.bids[std::cmp::min(20, depth.bids.len() - 1)].price;
    
                if ask_price > max_ask_price {
                    max_ask_price = ask_price;
                };
                if bid_price < min_bid_price {
                    min_bid_price = bid_price;
                };
            };
        };

        let lowest = min_bid_price - (min_bid_price * self.y_scaling);
        let highest = max_ask_price + (max_ask_price * self.y_scaling);

        (latest, earliest, highest, lowest)
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
            Message::YScaling(scaling) => {
                self.y_scaling = *scaling;
                self.render_start();
            }
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
                .horizontal_alignment(alignment::Horizontal::Center)
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(Message::AutoscaleToggle)
            .style(|_theme: &Theme, _status: iced::widget::button::Status| chart_button(_theme, _status, chart_state.autoscale));
        let crosshair_button = button(
            Text::new("+")
                .size(12)
                .horizontal_alignment(alignment::Horizontal::Center)
            ) 
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(Message::CrosshairToggle)
            .style(|_theme: &Theme, _status: iced::widget::button::Status| chart_button(_theme, _status, chart_state.crosshair));
    
        let chart_controls = Container::new(
            Row::new()
                .push(autoscale_button)
                .push(crosshair_button).spacing(2)
            ).padding([0, 2, 0, 2])
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
                        mouse::Button::Right => {
                            *interaction = Interaction::Drawing;
                            None
                        }
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
                        Interaction::Drawing => None,
                        Interaction::Erasing => None,
                        Interaction::Panning { translation, start } => {
                            Some(Message::Translated(
                                translation
                                    + (cursor_position - start)
                                        * (1.0 / chart_state.scaling),
                            ))
                        }
                        Interaction::None => 
                            if chart_state.crosshair && cursor.is_over(bounds) {
                                Some(Message::CrosshairMoved(cursor_position))
                            } else {
                                None
                            },
                    };

                    let event_status = match interaction {
                        Interaction::None => event::Status::Ignored,
                        _ => event::Status::Captured,
                    };

                    (event_status, message)
                }
                mouse::Event::WheelScrolled { delta } => match delta {
                    mouse::ScrollDelta::Lines { y, .. }
                    | mouse::ScrollDelta::Pixels { y, .. } => {
                        if y < 0.0 && chart_state.scaling > Self::MIN_SCALING
                            || y > 0.0 && chart_state.scaling < Self::MAX_SCALING
                        {
                            //let old_scaling = self.scaling;

                            let scaling = (chart_state.scaling * (1.0 + y / 30.0))
                                .clamp(
                                    Self::MIN_SCALING, 
                                    Self::MAX_SCALING,  
                                );

                            //let translation =
                            //    if let Some(cursor_to_center) =
                            //        cursor.position_from(bounds.center())
                            //    {
                            //        let factor = scaling - old_scaling;

                            //        Some(
                            //            self.translation
                            //                - Vector::new(
                            //                    cursor_to_center.x * factor
                            //                        / (old_scaling
                            //                            * old_scaling),
                            //                    cursor_to_center.y * factor
                            //                        / (old_scaling
                            //                            * old_scaling),
                            //                ),
                            //        )
                            //    } else {
                            //        None
                            //    };

                            (
                                event::Status::Captured,
                                Some(Message::Scaled(scaling, None)),
                            )
                        } else {
                            (event::Status::Captured, None)
                        }
                    }
                },
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
            let (mut min_trade_qty, mut max_trade_qty) = (f32::MAX, 0.0f32);

            let mut max_volume: f32 = 0.0;
        
            let mut max_depth_qty: f32 = 0.0;

            if self.data_points.len() > 1 {
                for (_, (depth, trades)) in self.data_points.range(earliest..=latest) {
                    let mut buy_volume: f32 = 0.0;
                    let mut sell_volume: f32 = 0.0;

                    for trade in trades.iter() {
                        max_trade_qty = max_trade_qty.max(trade.qty);
                        min_trade_qty = min_trade_qty.min(trade.qty);

                        if trade.is_sell {
                            sell_volume += trade.qty;
                        } else {
                            buy_volume += trade.qty;
                        }
                    }

                    max_volume = max_volume.max(buy_volume).max(sell_volume);
            
                    for ask in depth.asks.iter() {
                        if ask.price > highest {
                            continue;
                        };
                        max_depth_qty = max_depth_qty.max(ask.qty);
                    }
                    for bid in depth.bids.iter() {
                        if bid.price < lowest {
                            continue;
                        };
                        max_depth_qty = max_depth_qty.max(bid.qty);
                    }   
                };
                
                let mut prev_bid_price: Option<f32> = None;
                let mut prev_bid_qty: Option<f32> = None;
                let mut prev_ask_price: Option<f32> = None;
                let mut prev_ask_qty: Option<f32> = None;

                let mut prev_x_position: Option<f64> = None;

                for (time, (depth, trades)) in self.data_points.range(earliest..=latest) {
                    let x_position = ((time - earliest) as f64 / (latest - earliest) as f64) * bounds.width as f64;

                    let mut buy_volume: f32 = 0.0;
                    let mut sell_volume: f32 = 0.0;

                    for trade in trades.iter() {
                        if trade.is_sell {
                            sell_volume += trade.qty;
                        } else {
                            buy_volume += trade.qty;
                        }

                        if trade.qty * trade.price > self.size_filter {
                            let x_position = ((time - earliest) as f64 / (latest - earliest) as f64) * bounds.width as f64;
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

                            let circle = Path::circle(Point::new(x_position as f32, y_position), radius);
                            frame.fill(&circle, color);
                        }
                    }

                    for bid in depth.bids.iter() {
                        if bid.price >= lowest {
                            let y_position = heatmap_area_height - ((bid.price - lowest) / y_range * heatmap_area_height);
                            let color_alpha = (bid.qty / max_depth_qty).min(1.0);

                            if let (Some(prev_price), Some(prev_qty), Some(prev_x)) = (prev_bid_price, prev_bid_qty, prev_x_position) {
                                if prev_price != bid.price || prev_qty != bid.qty {
                                    let path = Path::line(Point::new(prev_x as f32, y_position), Point::new(x_position as f32, y_position));
                                    let stroke = Stroke::default().with_color(Color::from_rgba8(0, 144, 144, color_alpha)).with_width(1.0);
                                    frame.stroke(&path, stroke);
                                }
                            }
                            prev_bid_price = Some(bid.price);
                            prev_bid_qty = Some(bid.qty);
                        }
                    }

                    for ask in depth.asks.iter() {
                        if ask.price <= highest {
                            let y_position = heatmap_area_height - ((ask.price - lowest) / y_range * heatmap_area_height);
                            let color_alpha = (ask.qty / max_depth_qty).min(1.0);

                            if let (Some(prev_price), Some(prev_qty), Some(prev_x)) = (prev_ask_price, prev_ask_qty, prev_x_position) {
                                if prev_price != ask.price || prev_qty != ask.qty {
                                    let path = Path::line(Point::new(prev_x as f32, y_position), Point::new(x_position as f32, y_position));
                                    let stroke = Stroke::default().with_color(Color::from_rgba8(192, 0, 192, color_alpha)).with_width(1.0);
                                    frame.stroke(&path, stroke);
                                }
                            }
                            prev_ask_price = Some(ask.price);
                            prev_ask_qty = Some(ask.qty);
                        }
                    }

                    prev_x_position = Some(x_position);

                    if max_volume > 0.0 {
                        let buy_bar_height = (buy_volume / max_volume) * volume_area_height;
                        let sell_bar_height = (sell_volume / max_volume) * volume_area_height;

                        let sell_bar = Path::rectangle(
                            Point::new(x_position as f32, bounds.height - sell_bar_height), 
                            Size::new(1.0, sell_bar_height)
                        );
                        frame.fill(&sell_bar, Color::from_rgb8(192, 80, 77)); 

                        let buy_bar = Path::rectangle(
                            Point::new(x_position as f32 + 2.0, bounds.height - buy_bar_height), 
                            Size::new(1.0, buy_bar_height)
                        );
                        frame.fill(&buy_bar, Color::from_rgb8(81, 205, 160));
                    }
                };
            };
        
            // current orderbook as bars
            if let Some(latest_data_points) = self.data_points.iter().last() {
                let latest_timestamp = latest_data_points.0 + 200;

                let latest_bids: Vec<(f32, f32)> = latest_data_points.1.0.bids.iter()
                    .filter(|order| (order.price) >= lowest)
                    .map(|order| (order.price, order.qty))
                    .collect::<Vec<_>>();

                let latest_asks: Vec<(f32, f32)> = latest_data_points.1.0.asks.iter()
                    .filter(|order| (order.price) <= highest)
                    .map(|order| (order.price, order.qty))
                    .collect::<Vec<_>>();

                let max_qty = latest_bids.iter().map(|(_, qty)| qty).chain(latest_asks.iter().map(|(_, qty)| qty)).fold(f32::MIN, |arg0: f32, other: &f32| f32::max(arg0, *other));

                let x_position = ((latest_timestamp - earliest) as f32 / (latest - earliest) as f32) * bounds.width;

                for (price, qty) in &latest_bids {     
                    let y_position = heatmap_area_height - ((price - lowest) / y_range * heatmap_area_height);

                    let bar_width = (qty / max_qty) * depth_area_width;
                    let bar = Path::rectangle(
                        Point::new(x_position, y_position), 
                        Size::new(bar_width, 1.0) 
                    );
                    frame.fill(&bar, Color::from_rgba8(0, 144, 144, 0.5));
                }
                for (price, qty) in &latest_asks {
                    let y_position = heatmap_area_height - ((price - lowest) / y_range * heatmap_area_height);

                    let bar_width = (qty / max_qty) * depth_area_width; 
                    let bar = Path::rectangle(
                        Point::new(x_position, y_position), 
                        Size::new(bar_width, 1.0)
                    );
                    frame.fill(&bar, Color::from_rgba8(192, 0, 192, 0.5));
                }

                let line = Path::line(
                    Point::new(x_position, 0.0), 
                    Point::new(x_position, bounds.height)
                );
                frame.stroke(&line, Stroke::default().with_color(Color::from_rgba8(100, 100, 100, 0.1)).with_width(1.0));

                let text_size = 9.0;
                let text_content = format!("{max_qty:.2}");
                let text_position = Point::new(x_position + depth_area_width, 0.0);
                frame.fill_text(canvas::Text {
                    content: text_content,
                    position: text_position,
                    size: iced::Pixels(text_size),
                    color: Color::from_rgba8(81, 81, 81, 1.0),
                    ..canvas::Text::default()
                });

                let text_content = format!("{max_volume:.2}");
                if x_position > bounds.width {      
                    let text_width = (text_content.len() as f32 * text_size) / 1.5;

                    let text_position = Point::new(bounds.width - text_width, bounds.height - volume_area_height);
                    
                    frame.fill_text(canvas::Text {
                        content: text_content,
                        position: text_position,
                        size: iced::Pixels(text_size),
                        color: Color::from_rgba8(81, 81, 81, 1.0),
                        ..canvas::Text::default()
                    });

                } else {
                    let text_position = Point::new(x_position + 5.0, bounds.height - volume_area_height);

                    frame.fill_text(canvas::Text {
                        content: text_content,
                        position: text_position,
                        size: iced::Pixels(text_size),
                        color: Color::from_rgba8(81, 81, 81, 1.0),
                        ..canvas::Text::default()
                    });
                }
            };
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
            Interaction::Drawing => mouse::Interaction::Crosshair,
            Interaction::Erasing => mouse::Interaction::Crosshair,
            Interaction::Panning { .. } => mouse::Interaction::Grabbing,
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

const TIME_STEPS: [i64; 8] = [
    60 * 1000, // 1 minute
    30 * 1000, // 30 seconds
    15 * 1000, // 15 seconds
    10 * 1000, // 10 seconds
    5 * 1000,  // 5 seconds
    2 * 1000,  // 2 seconds
    1000,  // 1 second
    500,       // 500 milliseconds
];
fn calculate_time_step(earliest: i64, latest: i64, labels_can_fit: i32) -> (i64, i64) {
    let duration = latest - earliest;

    let mut selected_step = TIME_STEPS[0];
    for &step in &TIME_STEPS {
        if duration / step >= labels_can_fit as i64 {
            selected_step = step;
            break;
        }
    }

    let rounded_earliest = (earliest / selected_step) * selected_step;

    (selected_step, rounded_earliest)
}

pub struct AxisLabelXCanvas<'a> {
    labels_cache: &'a Cache,
    crosshair_cache: &'a Cache,
    crosshair_position: Point,
    crosshair: bool,
    min: i64,
    max: i64,
}
impl canvas::Program<Message> for AxisLabelXCanvas<'_> {
    type State = Interaction;

    fn update(
        &self,
        _interaction: &mut Interaction,
        _event: Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        (event::Status::Ignored, None)
    }
    
    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        if self.max == 0 {
            return vec![];
        }
        let latest_in_millis = self.max; 
        let earliest_in_millis = self.min; 

        let x_labels_can_fit = (bounds.width / 120.0) as i32;
        let (time_step, rounded_earliest) = calculate_time_step(self.min, self.max, x_labels_can_fit);

        let labels = self.labels_cache.draw(renderer, bounds.size(), |frame| {
            frame.with_save(|frame| {
                let mut time: i64 = rounded_earliest;
                let latest_time: i64 = latest_in_millis;

                while time <= latest_time {                    
                    let x_position = ((time - earliest_in_millis) as f64 / (latest_in_millis - earliest_in_millis) as f64) * bounds.width as f64;

                    if x_position >= 0.0 && x_position <= bounds.width as f64 {
                        let text_size = 12.0;
                        let time_as_datetime = NaiveDateTime::from_timestamp(time / 1000, 0);
                        let label = canvas::Text {
                            content: time_as_datetime.format("%M:%S").to_string(),
                            position: Point::new(x_position as f32 - text_size, bounds.height - 20.0),
                            size: iced::Pixels(text_size),
                            color: Color::from_rgba8(200, 200, 200, 1.0),
                            ..canvas::Text::default()
                        };  

                        label.draw_with(|path, color| {
                            frame.fill(&path, color);
                        });
                    }
                    
                    time += time_step;
                }

                let line = Path::line(
                    Point::new(0.0, bounds.height - 30.0), 
                    Point::new(bounds.width, bounds.height - 30.0)
                );
                frame.stroke(&line, Stroke::default().with_color(Color::from_rgba8(81, 81, 81, 0.2)).with_width(1.0));
            });
        });
        let crosshair = self.crosshair_cache.draw(renderer, bounds.size(), |frame| {
            if self.crosshair && self.crosshair_position.x > 0.0 {
                let crosshair_ratio = self.crosshair_position.x as f64 / bounds.width as f64;
                let crosshair_millis = (earliest_in_millis as f64 + crosshair_ratio * (latest_in_millis as f64 - earliest_in_millis as f64)).round() / 100.0 * 100.0;
                let crosshair_time = NaiveDateTime::from_timestamp((crosshair_millis / 1000.0).floor() as i64, ((crosshair_millis % 1000.0) * 1_000_000.0).round() as u32);
                
                let crosshair_timestamp = crosshair_time.timestamp_millis();

                let snap_ratio = (crosshair_timestamp as f64 - earliest_in_millis as f64) / (latest_in_millis as f64 - earliest_in_millis as f64);
                let snap_x = snap_ratio * bounds.width as f64;

                let text_size = 12.0;
                let text_content = crosshair_time.format("%M:%S:%3f").to_string().replace('.', "");
                let growth_amount = 6.0; 
                let rectangle_position = Point::new(snap_x as f32 - 26.0 - growth_amount, bounds.height - 20.0);
                let text_position = Point::new(snap_x as f32 - 26.0, bounds.height - 20.0);

                let text_background = canvas::Path::rectangle(rectangle_position, Size::new(text_content.len() as f32 * text_size/2.0 + 2.0 * growth_amount, text_size + text_size/2.0));
                frame.fill(&text_background, Color::from_rgba8(200, 200, 200, 1.0));

                let crosshair_label = canvas::Text {
                    content: text_content,
                    position: text_position,
                    size: iced::Pixels(text_size),
                    color: Color::from_rgba8(0, 0, 0, 1.0),
                    ..canvas::Text::default()
                };

                crosshair_label.draw_with(|path, color| {
                    frame.fill(&path, color);
                });
            }
        });

        vec![labels, crosshair]
    }

    fn mouse_interaction(
        &self,
        interaction: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match interaction {
            Interaction::Drawing => mouse::Interaction::Crosshair,
            Interaction::Erasing => mouse::Interaction::Crosshair,
            Interaction::Panning { .. } => mouse::Interaction::ResizingHorizontally,
            Interaction::None if cursor.is_over(bounds) => {
                mouse::Interaction::ResizingHorizontally
            }
            Interaction::None => mouse::Interaction::default(),
        }
    }
}