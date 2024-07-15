use std::collections::{BTreeMap, HashMap};
use iced::{
    alignment, mouse, widget::{button, canvas::{self, event::{self, Event}, stroke::Stroke, Canvas, Geometry, Path}}, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme
};
use iced::widget::{Column, Row, Container, Text};
use crate::data_providers::binance::market_data::{Kline, Trade};

use super::{Chart, CommonChartData, Message, Interaction, AxisLabelXCanvas, AxisLabelYCanvas};
use super::chart_button;

pub struct FootprintChart {
    chart: CommonChartData,
    data_points: BTreeMap<i64, (HashMap<i64, (f32, f32)>, (f32, f32, f32, f32, f32, f32))>,
    timeframe: u16,
    tick_size: f32,
    raw_trades: Vec<Trade>,
}

impl Chart for FootprintChart {
    type DataPoint = BTreeMap<i64, (HashMap<i64, (f32, f32)>, (f32, f32, f32, f32, f32, f32))>;

    fn get_common_data(&self) -> &CommonChartData {
        &self.chart
    }
    fn get_common_data_mut(&mut self) -> &mut CommonChartData {
        &mut self.chart
    }
}

impl FootprintChart {
    const MIN_SCALING: f32 = 0.4;
    const MAX_SCALING: f32 = 3.6;

    pub fn new(timeframe: u16, tick_size: f32, klines_raw: Vec<(i64, f32, f32, f32, f32, f32, f32)>, raw_trades: Vec<Trade>) -> Self {
        let mut data_points = BTreeMap::new();
        let aggregate_time = 1000 * 60 * timeframe as i64;

        for kline in klines_raw {
            let kline_raw = (kline.1, kline.2, kline.3, kline.4, kline.5, kline.6);
            data_points.entry(kline.0).or_insert((HashMap::new(), kline_raw));
        };
        for trade in &raw_trades {
            let rounded_time = (trade.time / aggregate_time) * aggregate_time;
            let price_level: i64 = (trade.price * (1.0 / tick_size)).round() as i64;

            let entry: &mut (HashMap<i64, (f32, f32)>, (f32, f32, f32, f32, f32, f32)) = data_points
                .entry(rounded_time)
                .or_insert((HashMap::new(), (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)));

            if let Some((buy_qty, sell_qty)) = entry.0.get_mut(&price_level) {
                if trade.is_sell {
                    *sell_qty += trade.qty;
                } else {
                    *buy_qty += trade.qty;
                }
            } else if trade.is_sell {
                entry.0.insert(price_level, (0.0, trade.qty));
            } else {
                entry.0.insert(price_level, (trade.qty, 0.0));
            }
        };

        FootprintChart {
            chart: CommonChartData::default(),
            data_points,
            timeframe,
            tick_size,
            raw_trades,
        }
    }

    pub fn insert_datapoint(&mut self, mut trades_buffer: Vec<Trade>, depth_update: i64) {
        let aggregate_time = 1000 * 60 * self.timeframe as i64;
        let rounded_depth_update = (depth_update / aggregate_time) * aggregate_time;
    
        self.data_points.entry(rounded_depth_update).or_insert((HashMap::new(), (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)));
        
        for trade in trades_buffer.drain(..) {
            let price_level: i64 = (trade.price * (1.0 / self.tick_size)).round() as i64;
            if let Some((trades, _)) = self.data_points.get_mut(&rounded_depth_update) {     
                if let Some((buy_qty, sell_qty)) = trades.get_mut(&price_level) {
                    if trade.is_sell {
                        *sell_qty += trade.qty;
                    } else {
                        *buy_qty += trade.qty;
                    }
                } else if trade.is_sell {
                    trades.insert(price_level, (0.0, trade.qty));
                } else {
                    trades.insert(price_level, (trade.qty, 0.0));
                }
            }

            self.raw_trades.push(trade);
        }
    }

    pub fn update_latest_kline(&mut self, kline: &Kline) {
        if let Some((_, kline_value)) = self.data_points.get_mut(&(kline.time as i64)) {
            kline_value.0 = kline.open;
            kline_value.1 = kline.high;
            kline_value.2 = kline.low;
            kline_value.3 = kline.close;
            kline_value.4 = kline.taker_buy_base_asset_volume;
            
            if kline_value.4 != -1.0 {
                kline_value.5 = kline.volume - kline.taker_buy_base_asset_volume;
            } else {
                kline_value.5 = kline.volume;
            }
        }

        self.render_start();
    }
    
    pub fn change_tick_size(&mut self, new_tick_size: f32) {
        let mut new_data_points = BTreeMap::new();
        let aggregate_time = 1000 * 60 * self.timeframe as i64;

        for (time, (_, kline_values)) in &self.data_points {
            new_data_points.entry(*time).or_insert((HashMap::new(), *kline_values));
        }

        for trade in self.raw_trades.iter() {
            let rounded_time = (trade.time / aggregate_time) * aggregate_time;
            let price_level: i64 = (trade.price * (1.0 / new_tick_size)).round() as i64;

            let entry = new_data_points
                .entry(rounded_time)
                .or_insert((HashMap::new(), (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)));

            if let Some((buy_qty, sell_qty)) = entry.0.get_mut(&price_level) {
                if trade.is_sell {
                    *sell_qty += trade.qty;
                } else {
                    *buy_qty += trade.qty;
                }
            } else if trade.is_sell {
                    entry.0.insert(price_level, (0.0, trade.qty));
            } else {
                entry.0.insert(price_level, (trade.qty, 0.0));
            }
        }
    
        self.data_points = new_data_points;
        self.tick_size = new_tick_size;
    }

    pub fn render_start(&mut self) {
        let (latest, earliest, highest, lowest) = self.calculate_range();
        if highest <= 0.0 || lowest <= 0.0 {
            return;
        }

        let chart_state = &mut self.chart;

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

    fn calculate_range(&self) -> (i64, i64, f32, f32) {
        let chart = self.get_common_data();

        let timestamp_latest = self.data_points.keys().last().unwrap_or(&0);

        let latest: i64 = *timestamp_latest - ((chart.translation.x*1000.0)*(self.timeframe as f32)) as i64;
        let earliest: i64 = latest - ((640000.0*self.timeframe as f32) / (chart.scaling / (chart.bounds.width/800.0))) as i64;
    
        let mut highest: f32 = 0.0;
        let mut lowest: f32 = std::f32::MAX;

        for (_, (_, kline)) in self.data_points.range(earliest..=latest) {
            if kline.1 > highest {
                highest = kline.1;
            }
            if kline.2 < lowest {
                lowest = kline.2;
            }
        }

        highest = highest + (highest - lowest) * 0.05;
        lowest = lowest - (highest - lowest) * 0.05;

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
            _ => {}
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
                timeframe: Some(self.timeframe)
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
                crosshair: chart_state.crosshair
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

impl canvas::Program<Message> for FootprintChart {
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
                                    Self::MIN_SCALING,  // 0.1
                                    Self::MAX_SCALING,  // 2.0
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

        let heatmap = chart.main_cache.draw(renderer, bounds.size(), |frame| {
            let mut max_trade_qty: f32 = 0.0;
            let mut max_volume: f32 = 0.0;

            for (_, (trades, kline)) in self.data_points.range(earliest..=latest) {
                for trade in trades {            
                    max_trade_qty = max_trade_qty.max(trade.1.0.max(trade.1.1));
                }
                max_volume = max_volume.max(kline.4.max(kline.5));
            }
            
            for (time, (trades, kline)) in self.data_points.range(earliest..=latest) {
                let x_position: f32 = ((time - earliest) as f32 / (latest - earliest) as f32) * bounds.width;

                if x_position.is_nan() || x_position.is_infinite() {
                    continue;
                }

                let y_open = heatmap_area_height - ((kline.0 - lowest) / y_range * heatmap_area_height);
                let y_high = heatmap_area_height - ((kline.1 - lowest) / y_range * heatmap_area_height);
                let y_low = heatmap_area_height - ((kline.2 - lowest) / y_range * heatmap_area_height);
                let y_close = heatmap_area_height - ((kline.3 - lowest) / y_range * heatmap_area_height);
                
                let body_color = if kline.3 >= kline.0 { Color::from_rgba8(81, 205, 160, 0.8) } else { Color::from_rgba8(192, 80, 77, 0.8) };
                let wick_color = if kline.3 >= kline.0 { Color::from_rgba8(81, 205, 160, 0.4) } else { Color::from_rgba8(192, 80, 77, 0.4) };

                let wick = Path::line(
                    Point::new(x_position, y_high), 
                    Point::new(x_position, y_low)
                );
                frame.stroke(&wick, Stroke::default().with_color(wick_color).with_width(1.0));

                let body = Path::rectangle(
                    Point::new(x_position - chart.scaling, y_open.min(y_close)), 
                    Size::new(2.0 * chart.scaling, (y_open - y_close).abs())
                );                    
                frame.fill(&body, body_color);

                for trade in trades {
                    let price = (*trade.0 as f32) / (1.0 / self.tick_size);
                    let y_position = heatmap_area_height - ((price - lowest) / y_range * heatmap_area_height);

                    if trade.1.0 > 0.0 {
                        let bar_width = (trade.1.0 / max_trade_qty) * bounds.width / 28.0 * chart.scaling;
                        let bar = Path::rectangle(
                            Point::new(x_position + (3.0 * chart.scaling), y_position), 
                            Size::new(bar_width, 1.0) 
                        );
                        frame.fill(&bar, Color::from_rgba8(81, 205, 160, 1.0));
                    } 
                    if trade.1.1 > 0.0 {
                        let bar_width = -(trade.1.1 / max_trade_qty) * bounds.width / 28.0 * chart.scaling;
                        let bar = Path::rectangle(
                            Point::new(x_position - (3.0 * chart.scaling), y_position), 
                            Size::new(bar_width, 1.0) 
                        );
                        frame.fill(&bar, Color::from_rgba8(192, 80, 77, 1.0));
                    };  
                }

                if max_volume > 0.0 {
                    if kline.4 != -1.0 {
                        let buy_bar_height = (kline.4 / max_volume) * volume_area_height;
                        let sell_bar_height = (kline.5 / max_volume) * volume_area_height;

                        let sell_bar_width = 8.0 * chart.scaling;
                        let sell_bar_x_position = x_position - (5.0*chart.scaling) - sell_bar_width;
                        let sell_bar = Path::rectangle(
                            Point::new(sell_bar_x_position, bounds.height - sell_bar_height), 
                            Size::new(sell_bar_width, sell_bar_height)
                        );
                        frame.fill(&sell_bar, Color::from_rgb8(192, 80, 77)); 

                        let buy_bar = Path::rectangle(
                            Point::new(x_position + (5.0*chart.scaling), bounds.height - buy_bar_height), 
                            Size::new(8.0 * chart.scaling, buy_bar_height)
                        );
                        frame.fill(&buy_bar, Color::from_rgb8(81, 205, 160));
                    } else {
                        let bar_height = (kline.5 / max_volume) * volume_area_height;
                        let bar = Path::rectangle(
                            Point::new(x_position - (3.0*chart.scaling), bounds.height - bar_height), 
                            Size::new(6.0 * chart.scaling, bar_height)
                        );
                        let color = if kline.3 >= kline.0 { Color::from_rgba8(81, 205, 160, 0.8) } else { Color::from_rgba8(192, 80, 77, 0.8) };

                        frame.fill(&bar, color);
                    }
                }
            } 
            
            let text_size = 9.0;
            let text_content = format!("{max_volume:.2}");
            let text_width = (text_content.len() as f32 * text_size) / 1.5;

            let text_position = Point::new(bounds.width - text_width, bounds.height - volume_area_height);
            
            frame.fill_text(canvas::Text {
                content: text_content,
                position: text_position,
                size: iced::Pixels(text_size),
                color: Color::from_rgba8(81, 81, 81, 1.0),
                ..canvas::Text::default()
            });
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
                    let crosshair_millis = earliest as f64 + crosshair_ratio * (latest - earliest) as f64;
                    let rounded_timestamp = (crosshair_millis / (self.timeframe as f64 * 60.0 * 1000.0)).round() as i64 * self.timeframe as i64 * 60 * 1000;

                    let snap_ratio = (rounded_timestamp as f64 - earliest as f64) / (latest as f64 - earliest as f64);
                    let snap_x = snap_ratio * bounds.width as f64;

                    let line = Path::line(
                        Point::new(snap_x as f32, 0.0), 
                        Point::new(snap_x as f32, bounds.height)
                    );
                    frame.stroke(&line, Stroke::default().with_color(Color::from_rgba8(200, 200, 200, 0.6)).with_width(1.0));

                    if let Some((_, kline)) = self.data_points.iter()
                        .find(|(time, _)| **time == rounded_timestamp) {

                            let tooltip_text: String = if kline.1.4 != -1.0 {
                                format!(
                                    "O: {} H: {} L: {} C: {}\nBuyV: {:.0} SellV: {:.0}",
                                    kline.1.0, kline.1.1, kline.1.2, kline.1.3, kline.1.4, kline.1.5
                                )
                            } else {
                                format!(
                                    "O: {} H: {} L: {} C: {}\nVolume: {:.0}",
                                    kline.1.0, kline.1.1, kline.1.2, kline.1.3, kline.1.5
                                )
                            };

                            let text = canvas::Text {
                                content: tooltip_text,
                                position: Point::new(10.0, 10.0),
                                size: iced::Pixels(12.0),
                                color: Color::from_rgba8(120, 120, 120, 1.0),
                                ..canvas::Text::default()
                            };
                            frame.fill_text(text);
                    }
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