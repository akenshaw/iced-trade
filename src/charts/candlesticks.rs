use std::collections::{BTreeMap, HashMap};
use chrono::NaiveDateTime;
use iced::{
    alignment, color, mouse, widget::{button, canvas::{self, event::{self, Event}, stroke::Stroke, Cache, Canvas, Geometry, Path}}, window, Border, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector
};
use iced::widget::{Column, Row, Container, Text};
use crate::{market_data::Kline, Timeframe};

use super::{Chart, CommonChartData, Message, chart_button};

pub struct Candlesticks {
    chart: CommonChartData,
    data_points: BTreeMap<i64, (f32, f32, f32, f32, f32, f32)>,
    timeframe: i16,
    mesh_cache: Cache,
}
impl Chart for Candlesticks {
    type DataPoint = BTreeMap<i64, (f32, f32, f32, f32, f32, f32)>;

    fn get_common_data(&self) -> &CommonChartData {
        &self.chart
    }
    fn get_common_data_mut(&mut self) -> &mut CommonChartData {
        &mut self.chart
    }
}

impl Candlesticks {
    const MIN_SCALING: f32 = 0.1;
    const MAX_SCALING: f32 = 2.0;

    pub fn new(klines: Vec<Kline>, timeframe: Timeframe) -> Candlesticks {
        let mut klines_raw = BTreeMap::new();

        for kline in klines {
            let buy_volume = kline.taker_buy_base_asset_volume;
            let sell_volume = kline.volume - buy_volume;
            klines_raw.insert(kline.time as i64, (kline.open, kline.high, kline.low, kline.close, buy_volume, sell_volume));
        }

        let timeframe = match timeframe {
            Timeframe::M1 => 1,
            Timeframe::M3 => 3,
            Timeframe::M5 => 5,
            Timeframe::M15 => 15,
            Timeframe::M30 => 30,
        };

        Candlesticks {
            chart: CommonChartData::default(),
            data_points: klines_raw,
            timeframe,
            mesh_cache: Cache::default(),
        }
    }

    pub fn insert_datapoint(&mut self, kline: &Kline) {
        let buy_volume: f32 = kline.taker_buy_base_asset_volume;
        let sell_volume: f32 = if buy_volume != -1.0 {
            kline.volume - buy_volume
        } else {
            kline.volume
        };

        self.data_points.insert(kline.time as i64, (kline.open, kline.high, kline.low, kline.close, buy_volume, sell_volume));

        self.render_start();
    }

    pub fn render_start(&mut self) {
        let (latest, earliest, highest, lowest) = self.calculate_range();

        if latest == 0 || highest == 0.0 {
            return;
        }

        let chart_state = &mut self.chart;

        if earliest != chart_state.x_min_time || latest != chart_state.x_max_time || lowest != chart_state.y_min_price || highest != chart_state.y_max_price {
            chart_state.x_labels_cache.clear();
            self.mesh_cache.clear();
        }

        chart_state.x_min_time = earliest;
        chart_state.x_max_time = latest;
        chart_state.y_min_price = lowest;
        chart_state.y_max_price = highest;

        chart_state.y_labels_cache.clear();
        chart_state.crosshair_cache.clear();
    }

    fn calculate_range(&self) -> (i64, i64, f32, f32) {
        let chart = self.get_common_data();

        let latest: i64 = self.data_points.keys().last().map_or(0, |time| time - ((chart.translation.x*10000.0)*(self.timeframe as f32)) as i64);
        let earliest: i64 = latest - ((6400000.0*self.timeframe as f32) / (chart.scaling / (chart.bounds.width/800.0))) as i64;

        let (visible_klines, highest, lowest, avg_body_height, _, _) = self.data_points.iter()
            .filter(|(time, _)| {
                **time >= earliest && **time <= latest
            })
            .fold((vec![], f32::MIN, f32::MAX, 0.0f32, 0.0f32, None), |(mut klines, highest, lowest, total_body_height, max_vol, latest_kline), (time, kline)| {
                let body_height = (kline.0 - kline.3).abs();
                klines.push((*time, *kline));
                let total_body_height = match latest_kline {
                    Some(_) => total_body_height + body_height,
                    None => total_body_height,
                };
                (
                    klines,
                    highest.max(kline.1),
                    lowest.min(kline.2),
                    total_body_height,
                    max_vol.max(kline.4.max(kline.5)),
                    Some(kline)
                )
            });

        if visible_klines.is_empty() || visible_klines.len() == 1 {
            return (0, 0, 0.0, 0.0);
        }

        let avg_body_height = avg_body_height / (visible_klines.len() - 1) as f32;
        let (highest, lowest) = (highest + avg_body_height, lowest - avg_body_height);

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
                timeframe: self.timeframe
            })
            .width(Length::FillPortion(10))
            .height(Length::Fixed(26.0));

        let last_close_price = self.data_points.values().last().map_or(0.0, |kline| kline.3);
        let last_open_price = self.data_points.values().last().map_or(0.0, |kline| kline.0);
    
        let axis_labels_y = Canvas::new(
            AxisLabelYCanvas { 
                labels_cache: &chart_state.y_labels_cache, 
                y_croshair_cache: &chart_state.y_crosshair_cache, 
                min: chart_state.y_min_price,
                max: chart_state.y_max_price,
                last_close_price, 
                last_open_price, 
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

#[derive(Debug, Clone, Copy)]
pub enum Interaction {
    None,
    Drawing,
    Erasing,
    Panning { translation: Vector, start: Point },
}

impl Default for Interaction {
    fn default() -> Self {
        Self::None
    }
}
impl canvas::Program<Message> for Candlesticks {
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

        let latest: i64 = self.data_points.keys().last().map_or(0, |time| time - ((chart.translation.x*10000.0)*(self.timeframe as f32)) as i64);
        let earliest: i64 = latest - ((6400000.0*self.timeframe as f32) / (chart.scaling / (bounds.width/800.0))) as i64;

        let (visible_klines, highest, lowest, avg_body_height, max_volume, _) = self.data_points.iter()
            .filter(|(time, _)| {
                **time >= earliest && **time <= latest
            })
            .fold((vec![], f32::MIN, f32::MAX, 0.0f32, 0.0f32, None), |(mut klines, highest, lowest, total_body_height, max_vol, latest_kline), (time, kline)| {
                let body_height = (kline.0 - kline.3).abs();
                klines.push((*time, *kline));
                let total_body_height = match latest_kline {
                    Some(_) => total_body_height + body_height,
                    None => total_body_height,
                };
                (
                    klines,
                    highest.max(kline.1),
                    lowest.min(kline.2),
                    total_body_height,
                    max_vol.max(kline.4.max(kline.5)),
                    Some(kline)
                )
            });

        if visible_klines.is_empty() || visible_klines.len() == 1 {
            return vec![];
        }

        let avg_body_height = avg_body_height / (visible_klines.len() - 1) as f32;
        let (highest, lowest) = (highest + avg_body_height, lowest - avg_body_height);
        let y_range = highest - lowest;

        let volume_area_height = bounds.height / 8.0; 
        let candlesticks_area_height = bounds.height - volume_area_height;

        let y_labels_can_fit = (bounds.height / 32.0) as i32;
        let (step, rounded_lowest) = calculate_price_step(highest, lowest, y_labels_can_fit);

        let x_labels_can_fit = (bounds.width / 90.0) as i32;
        let (time_step, rounded_earliest) = calculate_time_step(earliest, latest, x_labels_can_fit, self.timeframe);

        let background = self.mesh_cache.draw(renderer, bounds.size(), |frame| {
            frame.with_save(|frame| {
                let mut time = rounded_earliest;

                while time <= latest {                    
                    let x_position = ((time - earliest) as f64 / (latest - earliest) as f64) * bounds.width as f64;

                    if x_position >= 0.0 && x_position <= bounds.width as f64 {
                        let line = Path::line(
                            Point::new(x_position as f32, 0.0), 
                            Point::new(x_position as f32, bounds.height)
                        );
                        frame.stroke(&line, Stroke::default().with_color(Color::from_rgba8(27, 27, 27, 1.0)).with_width(1.0))
                    };
                    
                    time += time_step;
                }
            });
            
            frame.with_save(|frame| {
                let mut y = rounded_lowest;

                while y <= highest {
                    let y_position = candlesticks_area_height - ((y - lowest) / y_range * candlesticks_area_height);
                    let line = Path::line(
                        Point::new(0.0, y_position), 
                        Point::new(bounds.width, y_position)
                    );
                    frame.stroke(&line, Stroke::default().with_color(Color::from_rgba8(27, 27, 27, 1.0)).with_width(1.0));
                    y += step;
                }
            });
        });

        let candlesticks = chart.main_cache.draw(renderer, bounds.size(), |frame| {
            for (time, (open, high, low, close, buy_volume, sell_volume)) in visible_klines {
                let x_position: f64 = ((time - earliest) as f64 / (latest - earliest) as f64) * bounds.width as f64;
                
                let y_open = candlesticks_area_height - ((open - lowest) / y_range * candlesticks_area_height);
                let y_high = candlesticks_area_height - ((high - lowest) / y_range * candlesticks_area_height);
                let y_low = candlesticks_area_height - ((low - lowest) / y_range * candlesticks_area_height);
                let y_close = candlesticks_area_height - ((close - lowest) / y_range * candlesticks_area_height);
                
                let color = if close >= open { Color::from_rgb8(81, 205, 160) } else { Color::from_rgb8(192, 80, 77) };

                let body = Path::rectangle(
                    Point::new(x_position as f32 - (2.0 * chart.scaling), y_open.min(y_close)), 
                    Size::new(4.0 * chart.scaling, (y_open - y_close).abs())
                );                    
                frame.fill(&body, color);
                
                let wick = Path::line(
                    Point::new(x_position as f32, y_high), 
                    Point::new(x_position as f32, y_low)
                );
                frame.stroke(&wick, Stroke::default().with_color(color).with_width(1.0));

                if buy_volume != -1.0 {
                    let buy_bar_height = (buy_volume / max_volume) * volume_area_height;
                    let sell_bar_height = (sell_volume / max_volume) * volume_area_height;
                    
                    let buy_bar = Path::rectangle(
                        Point::new(x_position as f32, bounds.height - buy_bar_height), 
                        Size::new(2.0 * chart.scaling, buy_bar_height)
                    );
                    frame.fill(&buy_bar, Color::from_rgb8(81, 205, 160)); 
                    
                    let sell_bar = Path::rectangle(
                        Point::new(x_position as f32 - (2.0 * chart.scaling), bounds.height - sell_bar_height), 
                        Size::new(2.0 * chart.scaling, sell_bar_height)
                    );
                    frame.fill(&sell_bar, Color::from_rgb8(192, 80, 77)); 
                } else {
                    let bar_height = ((sell_volume) / max_volume) * volume_area_height;
                    
                    let bar = Path::rectangle(
                        Point::new(x_position as f32 - (2.0 * chart.scaling), bounds.height - bar_height), 
                        Size::new(4.0 * chart.scaling, bar_height)
                    );
                    let color = if close >= open { Color::from_rgba8(81, 205, 160, 0.8) } else { Color::from_rgba8(192, 80, 77, 0.8) };

                    frame.fill(&bar, color);
                }
            }
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
                        .find(|(time, _)| **time == rounded_timestamp as i64) {

                        
                        let tooltip_text: String = if kline.4 != -1.0 {
                            format!(
                                "O: {} H: {} L: {} C: {}\nBuyV: {:.0} SellV: {:.0}",
                                kline.0, kline.1, kline.2, kline.3, kline.4, kline.5
                            )
                        } else {
                            format!(
                                "O: {} H: {} L: {} C: {}\nVolume: {:.0}",
                                kline.0, kline.1, kline.2, kline.3, kline.5
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

            vec![background, crosshair, candlesticks]
        }   else {
            vec![background, candlesticks]
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

fn calculate_price_step(highest: f32, lowest: f32, labels_can_fit: i32) -> (f32, f32) {
    let range = highest - lowest;
    let mut step = 1000.0; 

    let steps = [1000.0, 500.0, 200.0, 100.0, 50.0, 20.0, 10.0, 5.0, 2.0, 1.0, 0.5, 0.2, 0.1, 0.05];

    for &s in steps.iter().rev() {
        if range / s <= labels_can_fit as f32 {
            step = s;
            break;
        }
    }
    let rounded_lowest = (lowest / step).floor() * step;

    (step, rounded_lowest)
}

const M1_TIME_STEPS: [i64; 9] = [
    1000 * 60 * 720, // 12 hour
    1000 * 60 * 180, // 3 hour
    1000 * 60 * 60, // 1 hour
    1000 * 60 * 30, // 30 minutes
    1000 * 60 * 15, // 15 minutes
    1000 * 60 * 10, // 10 minutes
    1000 * 60 * 5, // 5 minutes
    1000 * 60 * 2, // 2 minutes
    60 * 1000, // 1 minute
];
const M3_TIME_STEPS: [i64; 9] = [
    1000 * 60 * 1440, // 24 hour
    1000 * 60 * 720, // 12 hour
    1000 * 60 * 180, // 6 hour
    1000 * 60 * 120, // 6 hour
    1000 * 60 * 60, // 1 hour
    1000 * 60 * 30, // 30 minutes
    1000 * 60 * 15, // 15 minutes
    1000 * 60 * 9, // 9 minutes
    1000 * 60 * 3, // 3 minutes
];
const M5_TIME_STEPS: [i64; 9] = [
    1000 * 60 * 1440, // 24 hour
    1000 * 60 * 720, // 12 hour
    1000 * 60 * 480, // 8 hour
    1000 * 60 * 240, // 4 hour
    1000 * 60 * 120, // 2 hour
    1000 * 60 * 60, // 1 hour
    1000 * 60 * 30, // 30 minutes
    1000 * 60 * 15, // 15 minutes
    1000 * 60 * 5, // 5 minutes
];
fn calculate_time_step(earliest: i64, latest: i64, labels_can_fit: i32, timeframe: i16) -> (i64, i64) {
    let duration = latest - earliest;

    let time_steps = match timeframe {
        1 => &M1_TIME_STEPS,
        3 => &M3_TIME_STEPS,
        5 => &M5_TIME_STEPS,
        15 => &M5_TIME_STEPS[..7],
        30 => &M5_TIME_STEPS[..6],
        _ => &M1_TIME_STEPS,
    };

    let mut selected_step = time_steps[0];
    for &step in time_steps.iter() {
        if duration / step >= labels_can_fit as i64 {
            selected_step = step;
            break;
        }
        if step <= duration {
            selected_step = step;
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
    timeframe: i16,
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
        let x_labels_can_fit = (bounds.width / 90.0) as i32;
        let (time_step, rounded_earliest) = calculate_time_step(self.min, self.max, x_labels_can_fit, self.timeframe);

        let labels = self.labels_cache.draw(renderer, bounds.size(), |frame| {
            frame.with_save(|frame| {
                let mut time = rounded_earliest;

                while time <= self.max {                    
                    let x_position = ((time - self.min) as f64 / (self.max - self.min) as f64) * bounds.width as f64;

                    if x_position >= 0.0 && x_position <= bounds.width as f64 {
                        let text_size = 12.0;
                        let time_as_datetime = NaiveDateTime::from_timestamp(time / 1000, 0);
                        let label = canvas::Text {
                            content: time_as_datetime.format("%H:%M").to_string(),
                            position: Point::new(x_position as f32 - (text_size*4.0/3.0), bounds.height - 20.0),
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
            });
        });
        let crosshair = self.crosshair_cache.draw(renderer, bounds.size(), |frame| {
            if self.crosshair && self.crosshair_position.x > 0.0 {
                let crosshair_ratio = self.crosshair_position.x as f64 / bounds.width as f64;
                let crosshair_millis = self.min as f64 + crosshair_ratio * (self.max - self.min) as f64;
                let crosshair_time = NaiveDateTime::from_timestamp((crosshair_millis / 1000.0) as i64, 0);

                let crosshair_timestamp = crosshair_time.timestamp();
                let rounded_timestamp = (crosshair_timestamp as f64 / (self.timeframe as f64 * 60.0)).round() as i64 * self.timeframe as i64 * 60;
                let rounded_time = NaiveDateTime::from_timestamp(rounded_timestamp, 0);

                let snap_ratio = (rounded_timestamp as f64 * 1000.0 - self.min as f64) / (self.max as f64 - self.min as f64);
                let snap_x = snap_ratio * bounds.width as f64;

                let text_size: f32 = 12.0;
                let text_content: String = rounded_time.format("%H:%M").to_string();
                let growth_amount: f32 = 6.0; 
                let rectangle_position: Point = Point::new(snap_x as f32 - (text_size*4.0/3.0) - growth_amount, bounds.height - 20.0);
                let text_position: Point = Point::new(snap_x as f32 - (text_size*4.0/3.0), bounds.height - 20.0);

                let text_background = canvas::Path::rectangle(rectangle_position, Size::new(text_content.len() as f32 * text_size/2.0 + 2.0 * growth_amount + 1.0, text_size + text_size/2.0));
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
pub struct AxisLabelYCanvas<'a> {
    labels_cache: &'a Cache,
    y_croshair_cache: &'a Cache,
    min: f32,
    max: f32,
    last_close_price: f32,
    last_open_price: f32,
    crosshair_position: Point,
    crosshair: bool,
}
impl canvas::Program<Message> for AxisLabelYCanvas<'_> {
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
        if self.max == 0.0 {
            return vec![];
        }

        let y_labels_can_fit = (bounds.height / 32.0) as i32;
        let (step, rounded_lowest) = calculate_price_step(self.max, self.min, y_labels_can_fit);

        let volume_area_height = bounds.height / 8.0; 
        let candlesticks_area_height = bounds.height - volume_area_height;

        let labels = self.labels_cache.draw(renderer, bounds.size(), |frame| {
            frame.with_save(|frame| {
                let y_range = self.max - self.min;
                let mut y = rounded_lowest;

                while y <= self.max {
                    let y_position = candlesticks_area_height - ((y - self.min) / y_range * candlesticks_area_height);

                    let text_size = 12.0;
                    let decimal_places = i32::from(step.fract() != 0.0);
                    let label_content = match decimal_places {
                        0 => format!("{y:.0}"),
                        _ => format!("{y:.1}"),
                    };
                    let label = canvas::Text {
                        content: label_content,
                        position: Point::new(10.0, y_position - text_size / 2.0),
                        size: iced::Pixels(text_size),
                        color: Color::from_rgba8(200, 200, 200, 1.0),
                        ..canvas::Text::default()
                    };  

                    label.draw_with(|path, color| {
                        frame.fill(&path, color);
                    });

                    y += step;
                }

                let last_close_y_position = candlesticks_area_height - ((self.last_close_price - self.min) / y_range * candlesticks_area_height);

                let triangle_color = if self.last_close_price >= self.last_open_price {
                    Color::from_rgba8(81, 205, 160, 0.9) 
                } else {
                    Color::from_rgba8(192, 80, 77, 0.9) 
                };

                let triangle = Path::new(|path| {
                    path.move_to(Point::new(5.0, last_close_y_position));
                    path.line_to(Point::new(0.0, last_close_y_position - 5.0));
                    path.line_to(Point::new(0.0, last_close_y_position + 5.0));
                    path.close();
                });

                frame.fill(&triangle, triangle_color);
            });
        });
        let crosshair = self.y_croshair_cache.draw(renderer, bounds.size(), |frame| {
            if self.crosshair && self.crosshair_position.y > 0.0 {
                let text_size = 12.0;
                let y_range = self.max - self.min;
                let label_content = format!("{:.1}", self.min + (y_range * (candlesticks_area_height - self.crosshair_position.y) / candlesticks_area_height));
                
                let growth_amount = 3.0; 
                let rectangle_position = Point::new(8.0 - growth_amount, self.crosshair_position.y - text_size / 2.0 - 3.0);
                let text_position = Point::new(8.0, self.crosshair_position.y - text_size / 2.0 - 3.0);

                let text_background = canvas::Path::rectangle(rectangle_position, Size::new(label_content.len() as f32 * text_size / 2.0 + 2.0 * growth_amount + 4.0, text_size + text_size / 1.8));
                frame.fill(&text_background, Color::from_rgba8(200, 200, 200, 1.0));

                let label = canvas::Text {
                    content: label_content,
                    position: text_position,
                    size: iced::Pixels(text_size),
                    color: Color::from_rgba8(0, 0, 0, 1.0),
                    ..canvas::Text::default()
                };

                label.draw_with(|path, color| {
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
            Interaction::Panning { .. } => mouse::Interaction::ResizingVertically,
            Interaction::None if cursor.is_over(bounds) => {
                mouse::Interaction::ResizingVertically
            }
            Interaction::None => mouse::Interaction::default(),
        }
    }
}