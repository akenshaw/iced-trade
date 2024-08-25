use std::collections::{BTreeMap, HashMap};
use iced::{
    alignment, mouse, widget::{button, canvas::{self, event::{self, Event}, stroke::Stroke, Canvas, Geometry, Path}}, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector
};
use iced::widget::{Column, Row, Container, Text};
use iced_futures::core::time;
use crate::data_providers::{Kline, Trade};

use super::{Chart, CommonChartData, Message, Interaction, AxisLabelXCanvas, AxisLabelYCanvas};
use super::chart_button;

impl Chart for FootprintChart {
    type DataPoint = BTreeMap<i64, (HashMap<i64, (f32, f32)>, Kline)>;

    fn get_common_data(&self) -> &CommonChartData {
        &self.chart
    }
    fn get_common_data_mut(&mut self) -> &mut CommonChartData {
        &mut self.chart
    }
}

#[derive(Debug, Clone)]
pub struct Region {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

pub struct FootprintChart {
    chart: CommonChartData,
    data_points: BTreeMap<i64, (HashMap<i64, (f32, f32)>, Kline)>,
    timeframe: u16,
    tick_size: f32,
    raw_trades: Vec<Trade>,
    cell_width: f32,
    cell_height: f32,
    highest_y: f32,
    lowest_y: f32,
}

impl FootprintChart {
    const MIN_SCALING: f32 = 0.1;
    const MAX_SCALING: f32 = 2.0;

    const MAX_CELL_WIDTH: f32 = 120.0;
    const MIN_CELL_WIDTH: f32 = 12.0;

    const MAX_CELL_HEIGHT: f32 = 80.0;
    const MIN_CELL_HEIGHT: f32 = 8.0;

    pub fn new(timeframe: u16, tick_size: f32, klines_raw: Vec<Kline>, raw_trades: Vec<Trade>) -> Self {
        let mut data_points = BTreeMap::new();
        let aggregate_time = 1000 * 60 * timeframe as i64;

        let (mut highest_y, mut lowest_y) = (0.0f32, std::f32::MAX);

        for kline in klines_raw {
            data_points.entry(kline.time as i64).or_insert((HashMap::new(), kline));

            if kline.low < lowest_y {
                lowest_y = kline.low;
            }
            if kline.high > highest_y {
                highest_y = kline.high;
            }
        };
        for trade in &raw_trades {
            let rounded_time = (trade.time / aggregate_time) * aggregate_time;
            let price_level: i64 = (trade.price * (1.0 / tick_size)).round() as i64;

            let entry = data_points
                .entry(rounded_time)
                .or_insert((HashMap::new(), Kline::default()));

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
            cell_width: timeframe as f32 * 60.0,
            cell_height: 8.0 * tick_size,
            highest_y,
            lowest_y,
        }
    }

    pub fn insert_datapoint(&mut self, trades_buffer: &[Trade], depth_update: i64) {
        let aggregate_time = 1000 * 60 * self.timeframe as i64;
        let rounded_depth_update = (depth_update / aggregate_time) * aggregate_time;
    
        self.data_points.entry(rounded_depth_update).or_insert((HashMap::new(), Kline::default()));
        
        for trade in trades_buffer {
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

            self.raw_trades.push(*trade);
        }
    }

    pub fn update_latest_kline(&mut self, kline: &Kline) {
        if let Some((_, kline_value)) = self.data_points.get_mut(&(kline.time as i64)) {
            kline_value.open = kline.open;
            kline_value.high = kline.high;
            kline_value.low = kline.low;
            kline_value.close = kline.close;
            kline_value.volume = kline.volume;
        } 

        if kline.low < self.lowest_y {
            self.lowest_y = kline.low;
        }
        if kline.high > self.highest_y {
            self.highest_y = kline.high;
        }

        self.render_start();
    }

    pub fn get_raw_trades(&self) -> Vec<Trade> {
        self.raw_trades.clone()
    }
    
    pub fn get_tick_size(&self) -> f32 {
        self.tick_size
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
                .or_insert((HashMap::new(), Kline::default()));

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

        self.cell_height = 8.0 * new_tick_size;
    }

    pub fn render_start(&mut self) {
        let chart_state = &mut self.chart;
    
        chart_state.crosshair_cache.clear();
        chart_state.main_cache.clear();
    }

    fn visible_region(&self, size: Size) -> Region {
        let chart = self.get_common_data();

        let width = size.width / chart.scaling;
        let height = size.height / chart.scaling;

        Region {
            x: -chart.translation.x - width / 2.0,
            y: -chart.translation.y - height / 2.0,
            width,
            height,
        }
    }

    fn time_to_x(&self, time: i64) -> f32 {
        let time_per_cell = self.timeframe as i64 * 60 * 1000; 
        let latest_time = *self.data_points.last_key_value().unwrap().0;
        
        ((time - latest_time) as f32 / time_per_cell as f32) * self.cell_width
    }

    fn x_to_time(&self, x: f32) -> i64 {
        let time_per_cell = self.timeframe as i64 * 60 * 1000; 
        let latest_time = *self.data_points.last_key_value().unwrap().0;
        
        latest_time + ((x / self.cell_width) * time_per_cell as f32) as i64
    }

    fn price_to_y(&self, price: f32) -> f32 {        
        ((self.lowest_y - price) / self.tick_size) * self.cell_height
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
            Message::XScaling(delta, cursor_to_center_x, is_wheel_scroll) => {
                if *delta < 0.0 && self.cell_width > Self::MIN_CELL_WIDTH || *delta > 0.0 && self.cell_width < Self::MAX_CELL_WIDTH {
                    let old_width = self.cell_width;

                    let cell_width = (self.cell_width * (1.0 + delta / 30.0))
                        .clamp(
                            Self::MIN_CELL_WIDTH, 
                            Self::MAX_CELL_WIDTH,  
                        );

                    let chart_state = self.get_common_data_mut();

                    let factor = cell_width - old_width;

                    chart_state.translation.x -= (cursor_to_center_x * factor) / (old_width * old_width);

                    self.cell_width = cell_width;

                    self.render_start();
                }
            },
            Message::YScaling(delta, cursor_to_center_y, is_wheel_scroll) => {
                if *delta < 0.0 && self.cell_height > Self::MIN_CELL_HEIGHT || *delta > 0.0 && self.cell_height < Self::MAX_CELL_HEIGHT {
                    let old_height = self.cell_height;

                    let cell_height = (self.cell_height * (1.0 + delta / 30.0))
                        .clamp(
                            Self::MIN_CELL_HEIGHT, 
                            Self::MAX_CELL_HEIGHT,  
                        );

                    let chart_state = self.get_common_data_mut();

                    let factor = cell_height - old_height;

                    chart_state.translation.y -= (cursor_to_center_y * factor) / (old_height * old_height);

                    self.cell_height = cell_height;

                    self.render_start();
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
                            Some(Message::Translated(
                                translation + (cursor_position - start) * (1.0 / chart_state.scaling),
                            ))
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
                        if y < 0.0 && chart_state.scaling > Self::MIN_SCALING || y > 0.0 && chart_state.scaling < Self::MAX_SCALING {
                            let old_scaling = chart_state.scaling;

                            let scaling = (chart_state.scaling * (1.0 + y / 30.0))
                                .clamp(
                                    Self::MIN_SCALING,  // 0.1
                                    Self::MAX_SCALING,  // 2.0
                                );

                            let translation =
                                if let Some(cursor_to_center) =
                                    cursor.position_from(bounds.center())
                                {
                                    let factor = scaling - old_scaling;
                                    Some(
                                        chart_state.translation
                                            - Vector::new(
                                                cursor_to_center.x * factor
                                                    / (old_scaling
                                                        * old_scaling),
                                                cursor_to_center.y * factor
                                                    / (old_scaling
                                                        * old_scaling),
                                            ),
                                    )
                                } else {
                                    None
                                };

                            (
                                event::Status::Captured,
                                Some(Message::Scaled(scaling, translation)),
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

        let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

        let (cell_width, cell_height) = (self.cell_width, self.cell_height);

        let footprint = chart.main_cache.draw(renderer, bounds.size(), |frame| {
            frame.with_save(|frame| {                
                frame.translate(center);
                frame.scale(chart.scaling);
                frame.translate(chart.translation);

                let region = self.visible_region(frame.size());
        
                if !self.data_points.is_empty() {
                    let earliest = self.x_to_time(region.x);
                    let latest = self.x_to_time(region.x + region.width);

                    let mut max_trade_qty: f32 = 0.0;
                    let mut max_volume: f32 = 0.0;
                    
                    for (&timestamp, (trades, kline)) in self.data_points.range(earliest..=latest).rev() {
                        let x_position = self.time_to_x(timestamp);

                        if x_position >= region.x && x_position <= region.x + region.width {
                            for trade in trades {
                                max_trade_qty = max_trade_qty.max(trade.1.0.max(trade.1.1));
                            }
                            max_volume = max_volume.max(kline.volume.0.max(kline.volume.1));
                        }
                    }

                    for (&timestamp, (trades, kline)) in self.data_points.range(earliest..=latest).rev() {
                        let x_position = self.time_to_x(timestamp);

                        let y_open = self.price_to_y(kline.open);
                        let y_high = self.price_to_y(kline.high);
                        let y_low = self.price_to_y(kline.low);
                        let y_close = self.price_to_y(kline.close);

                        let candle_width = 0.2 * cell_width;

                        let body_color = 
                            if kline.close >= kline.open { 
                                Color::from_rgba8(81, 205, 160, 0.8) 
                            } else { Color::from_rgba8(192, 80, 77, 0.8) 
                        };
                        frame.fill_rectangle(
                            Point::new(x_position - (candle_width / 4.0), y_open.min(y_close)), 
                            Size::new(candle_width / 2.0, (y_open - y_close).abs()), 
                            body_color
                        );

                        let wick_color = 
                            if kline.close >= kline.open { 
                                Color::from_rgba8(81, 205, 160, 0.4) 
                            } else { Color::from_rgba8(192, 80, 77, 0.4) 
                        };
                        frame.fill_rectangle(
                            Point::new(x_position - 1.0, y_high),
                            Size::new(2.0, (y_high - y_low).abs()),
                            wick_color
                        );

                        for trade in trades {
                            let price = (*trade.0 as f32) / (1.0 / self.tick_size);
                            let y_position = self.price_to_y(price);
        
                            if trade.1.0 > 0.0 {
                                let bar_width = (trade.1.0 / max_trade_qty) * (cell_height * 0.4);
        
                                frame.fill_rectangle(
                                    Point::new(x_position + (candle_width / 3.0), y_position), 
                                    Size::new(bar_width, cell_height) , 
                                    Color::from_rgba8(81, 205, 160, 1.0)
                                );
                            } 
                            if trade.1.1 > 0.0 {
                                let bar_width = -(trade.1.1 / max_trade_qty) * ((cell_height * 0.4));
        
                                frame.fill_rectangle(
                                    Point::new(x_position - (candle_width / 3.0), y_position), 
                                    Size::new(bar_width, cell_height), 
                                    Color::from_rgba8(192, 80, 77, 1.0)
                                );
                            }
                        }
                    }
                }
            });
        });

        vec![footprint]
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