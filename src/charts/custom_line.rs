use std::collections::BTreeMap;
use chrono::{DateTime, Utc, TimeZone, LocalResult, Duration, NaiveDateTime, Timelike};
use iced::{
    mouse, 
    widget::canvas::{self, event::{self, Event}, 
    stroke::Stroke, Cache, Geometry, Path, Canvas}, 
    window, Color, Point, Rectangle, Renderer, Size, Theme, Vector, Element, Length
};
use iced::widget::{Column, Row, Container, Space};
use crate::market_data::Kline;

#[derive(Debug, Clone)]
pub enum Message {
    Translated(Vector),
    Scaled(f32, Option<Vector>),
    ChartBounds(f32, f32),
}

#[derive(Debug)]
pub struct CustomLine {
    mesh_cache: Cache,
    candles_cache: Cache,
    x_labels_cache: Cache,
    y_labels_cache: Cache,
    translation: Vector,
    scaling: f32,
    klines_raw: BTreeMap<DateTime<Utc>, (f32, f32, f32, f32, f32, f32)>,
    autoscale: bool,
    x_min_time: i64,
    x_max_time: i64,
    y_min_price: f32,
    y_max_price: f32,
    chart_width: f32,
    chart_height: f32,
}
impl CustomLine {
    const MIN_SCALING: f32 = 0.1;
    const MAX_SCALING: f32 = 2.0;

    pub fn new(_klines: Vec<Kline>, _timeframe_in_minutes: i16) -> CustomLine {
        let _size = window::Settings::default().size;
        CustomLine {
            mesh_cache: canvas::Cache::default(),
            candles_cache: canvas::Cache::default(),
            x_labels_cache: canvas::Cache::default(),
            y_labels_cache: canvas::Cache::default(),
            klines_raw: BTreeMap::new(),
            translation: Vector::default(),
            scaling: 1.0,
            autoscale: true,
            x_min_time: 0,
            x_max_time: 0,
            y_min_price: 0.0,
            y_max_price: 0.0,
            chart_width: 0.0,
            chart_height: 0.0,
        }
    }

    pub fn set_dataset(&mut self, klines: Vec<Kline>) {
        self.klines_raw.clear();

        for kline in klines {
            let time = match Utc.timestamp_opt(kline.time as i64 / 1000, 0) {
                LocalResult::Single(dt) => dt,
                _ => continue, 
            };
            let buy_volume = kline.taker_buy_base_asset_volume;
            let sell_volume = kline.volume - buy_volume;
            self.klines_raw.insert(time, (kline.open, kline.high, kline.low, kline.close, buy_volume, sell_volume));
        }
    }

    pub fn insert_datapoint(&mut self, kline: Kline) {
        let time = match Utc.timestamp_opt(kline.time as i64 / 1000, 0) {
            LocalResult::Single(dt) => dt,
            _ => return, 
        };
        let buy_volume = kline.taker_buy_base_asset_volume;
        let sell_volume = kline.volume - buy_volume;
        self.klines_raw.insert(time, (kline.open, kline.high, kline.low, kline.close, buy_volume, sell_volume));

        self.render_start();
    }
    
    pub fn render_start(&mut self) {
        let latest: i64 = self.klines_raw.keys().last().map_or(0, |time| time.timestamp() - (self.translation.x*10.0) as i64);
        let earliest: i64 = latest - (6400.0 / (self.scaling / (self.chart_width/800.0))) as i64;
    
        let (visible_klines, highest, lowest, avg_body_height, _) = self.klines_raw.iter()
            .filter(|(time, _)| {
                let timestamp = time.timestamp();
                timestamp >= earliest && timestamp <= latest
            })
            .fold((vec![], f32::MIN, f32::MAX, 0.0f32, 0.0f32), |(mut klines, highest, lowest, total_body_height, max_vol), (time, kline)| {
                let body_height = (kline.0 - kline.3).abs();
                klines.push((*time, *kline));
                (
                    klines,
                    highest.max(kline.1),
                    lowest.min(kline.2),
                    total_body_height + body_height,
                    max_vol.max(kline.4.max(kline.5)) 
                )
            });
    
        if visible_klines.is_empty() {
            return;
        }
    
        let avg_body_height = avg_body_height / visible_klines.len() as f32;
        let (highest, lowest) = (highest + avg_body_height, lowest - avg_body_height);

        self.x_min_time = earliest;
        self.x_max_time = latest;
        self.y_min_price = lowest;
        self.y_max_price = highest;

        self.x_labels_cache.clear();
        self.y_labels_cache.clear();
        self.mesh_cache.clear();
        self.candles_cache.clear();
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Translated(translation) => {
                if self.autoscale {
                    self.translation.x = translation.x;
                } else {
                    self.translation = translation;
                }

                self.render_start();
            }
            Message::Scaled(scaling, translation) => {
                self.scaling = scaling;
                
                if let Some(translation) = translation {
                    if self.autoscale {
                        self.translation.x = translation.x;
                    } else {
                        self.translation = translation;
                    }
                }

                self.render_start();
            }
            Message::ChartBounds(width, height) => {
                self.chart_width = width;
                self.chart_height = height;
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let chart = Canvas::new(self)
            .width(Length::FillPortion(10))
            .height(Length::FillPortion(10));
    
        let axis_labels_x = Canvas::new(
            AxisLabelXCanvas { 
                labels_cache: &self.x_labels_cache, min: self.x_min_time, max: self.x_max_time 
            })
            .width(Length::FillPortion(10))
            .height(Length::Fixed(25.0));
    
        let axis_labels_y = Canvas::new(
            AxisLabelYCanvas { 
                labels_cache: &self.y_labels_cache, min: self.y_min_price, max: self.y_max_price 
            })
            .width(Length::Fixed(40.0))
            .height(Length::FillPortion(10));
    
        let empty_space = Container::new(Space::new(Length::Fixed(40.0), Length::Fixed(40.0)))
            .width(Length::Fixed(40.0))
            .height(Length::Fixed(25.0));
    
        let chart_and_y_labels = Row::new()
            .push(chart)
            .push(axis_labels_y)
            .spacing(0);
    
        let bottom_row = Row::new()
            .push(axis_labels_x)
            .push(empty_space)
            .spacing(0);
    
        let content = Column::new()
            .push(chart_and_y_labels)
            .push(bottom_row)
            .spacing(0);
    
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
impl canvas::Program<Message> for CustomLine {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {        
        if bounds.width != self.chart_width || bounds.height != self.chart_height {
            return (event::Status::Ignored, Some(Message::ChartBounds(bounds.width, bounds.height)));
        } 
        
        if let Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
            *interaction = Interaction::None;
        }

        let Some(cursor_position) = cursor.position_in(bounds) else {
            return (event::Status::Ignored, None);
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
                                translation: self.translation,
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
                                        * (1.0 / self.scaling),
                            ))
                        }
                        Interaction::None => None,
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
                        if y < 0.0 && self.scaling > Self::MIN_SCALING
                            || y > 0.0 && self.scaling < Self::MAX_SCALING
                        {
                            //let old_scaling = self.scaling;

                            let scaling = (self.scaling * (1.0 + y / 30.0))
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
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {    
        let latest: i64 = self.klines_raw.keys().last().map_or(0, |time| time.timestamp() - (self.translation.x*10.0) as i64);
        let earliest: i64 = latest - (6400.0 / (self.scaling / (bounds.width/800.0))) as i64;
    
        let (visible_klines, highest, lowest, avg_body_height, max_volume) = self.klines_raw.iter()
            .filter(|(time, _)| {
                let timestamp = time.timestamp();
                timestamp >= earliest && timestamp <= latest
            })
            .fold((vec![], f32::MIN, f32::MAX, 0.0f32, 0.0f32), |(mut klines, highest, lowest, total_body_height, max_vol), (time, kline)| {
                let body_height = (kline.0 - kline.3).abs();
                klines.push((*time, *kline));
                (
                    klines,
                    highest.max(kline.1),
                    lowest.min(kline.2),
                    total_body_height + body_height,
                    max_vol.max(kline.4.max(kline.5)) 
                )
            });
    
        if visible_klines.is_empty() {
            return vec![];
        }
    
        let avg_body_height = avg_body_height / visible_klines.len() as f32;
        let (highest, lowest) = (highest + avg_body_height, lowest - avg_body_height);
        let y_range = highest - lowest;

        let volume_area_height = bounds.height / 8.0; 
        let candlesticks_area_height = bounds.height - volume_area_height;

        let y_labels_can_fit = (bounds.height / 60.0) as i32;
        let (step, rounded_lowest) = calculate_price_step(highest, lowest, y_labels_can_fit);

        let x_labels_can_fit = (bounds.width / 100.0) as i32;
        let (time_step, rounded_earliest) = calculate_time_step(earliest, latest, x_labels_can_fit);

        let background = self.mesh_cache.draw(renderer, bounds.size(), |frame| {
            frame.with_save(|frame| {
                let latest_in_millis = latest * 1000; 
                let earliest_in_millis = earliest * 1000; 

                let mut time = rounded_earliest;
                let latest_time = NaiveDateTime::from_timestamp(latest, 0);

                while time <= latest_time {
                    let time_in_millis = time.timestamp_millis();
                    
                    let x_position = ((time_in_millis - earliest_in_millis) as f64 / (latest_in_millis - earliest_in_millis) as f64) * bounds.width as f64;

                    if x_position >= 0.0 && x_position <= bounds.width as f64 {
                        let line = Path::line(
                            Point::new(x_position as f32, 0.0), 
                            Point::new(x_position as f32, bounds.height as f32)
                        );
                        frame.stroke(&line, Stroke::default().with_color(Color::from_rgba8(40, 40, 40, 1.0)).with_width(1.0))
                    }
                    
                    time = time + time_step;
                }
            });
            
            frame.with_save(|frame| {
                let mut y = rounded_lowest;

                while y <= highest {
                    let y_position = candlesticks_area_height - ((y - lowest) / y_range * candlesticks_area_height);
                    let line = Path::line(
                        Point::new(0.0, y_position), 
                        Point::new(bounds.width as f32, y_position)
                    );
                    frame.stroke(&line, Stroke::default().with_color(Color::from_rgba8(40, 40, 40, 1.0)).with_width(1.0));
                    y += step;
                }
            });
        });

        let candlesticks = 
            self.candles_cache.draw(renderer, bounds.size(), |frame| {
                for (time, (open, high, low, close, buy_volume, sell_volume)) in visible_klines {
                    let x_position: f64 = ((time.timestamp() - earliest) as f64 / (latest - earliest) as f64) * bounds.width as f64;
                    
                    let y_open = candlesticks_area_height - ((open - lowest) / y_range * candlesticks_area_height);
                    let y_high = candlesticks_area_height - ((high - lowest) / y_range * candlesticks_area_height);
                    let y_low = candlesticks_area_height - ((low - lowest) / y_range * candlesticks_area_height);
                    let y_close = candlesticks_area_height - ((close - lowest) / y_range * candlesticks_area_height);
                    
                    let color = if close >= open { Color::from_rgb8(81, 205, 160) } else { Color::from_rgb8(192, 80, 77) };

                    let body = Path::rectangle(
                        Point::new(x_position as f32 - (2.0 * self.scaling), y_open.min(y_close)), 
                        Size::new(4.0 * self.scaling, (y_open - y_close).abs())
                    );                    
                    frame.fill(&body, color);
                    
                    let wick = Path::line(
                        Point::new(x_position as f32, y_high), 
                        Point::new(x_position as f32, y_low)
                    );
                    frame.stroke(&wick, Stroke::default().with_color(color).with_width(1.0));

                    let buy_bar_height = (buy_volume / max_volume) * volume_area_height;
                    let sell_bar_height = (sell_volume / max_volume) * volume_area_height;
                    
                    let buy_bar = Path::rectangle(
                        Point::new(x_position as f32, (bounds.height - buy_bar_height) as f32), 
                        Size::new(2.0 * self.scaling, buy_bar_height as f32)
                    );
                    frame.fill(&buy_bar, Color::from_rgb8(81, 205, 160)); 
                    
                    let sell_bar = Path::rectangle(
                        Point::new(x_position as f32 - (2.0 * self.scaling), (bounds.height - sell_bar_height) as f32), 
                        Size::new(2.0 * self.scaling, sell_bar_height as f32)
                    );
                    frame.fill(&sell_bar, Color::from_rgb8(192, 80, 77)); 
                }
            });

        vec![background, candlesticks]
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
                mouse::Interaction::Crosshair
            }
            Interaction::None => mouse::Interaction::default(),
        }
    }
}

fn calculate_price_step(highest: f32, lowest: f32, labels_can_fit: i32) -> (f32, f32) {
    let range = highest - lowest;
    let mut step = 100.0; 

    let steps = [100.0, 50.0, 20.0, 10.0, 5.0, 2.0, 1.0, 0.5];

    for &s in steps.iter().rev() {
        if range / s <= labels_can_fit as f32 {
            step = s;
            break;
        }
    }
    let rounded_lowest = (lowest / step).floor() * step;

    (step, rounded_lowest)
}
fn calculate_time_step(earliest: i64, latest: i64, labels_can_fit: i32) -> (Duration, NaiveDateTime) {
    let duration = latest - earliest;
    let duration_in_millis = duration * 1000; 

    let steps = [
        Duration::minutes(60),
        Duration::minutes(30),
        Duration::minutes(15),
        Duration::minutes(10),
        Duration::minutes(5),
        Duration::minutes(1),
    ];

    let mut selected_step = steps[0];
    for &step in steps.iter() {
        if duration_in_millis / step.num_milliseconds() >= labels_can_fit as i64 {
            selected_step = step;
            break;
        }
    }

    let mut rounded_earliest = NaiveDateTime::from_timestamp(earliest, 0)
        .with_second(0).unwrap()
        .with_nanosecond(0).unwrap();

    let minutes = rounded_earliest.minute();
    let step_minutes = selected_step.num_minutes() as u32;
    let remainder = minutes % step_minutes;
    if remainder > 0 {
        rounded_earliest = rounded_earliest + Duration::minutes((step_minutes - remainder) as i64);
    }

    (selected_step, rounded_earliest)
}

impl Default for CustomLine {
    fn default() -> Self {
        Self::new(vec![], 1)
    }
}
pub struct AxisLabelXCanvas<'a> {
    labels_cache: &'a Cache,
    min: i64,
    max: i64,
}
impl canvas::Program<Message> for AxisLabelXCanvas<'_> {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
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

        let x_labels_can_fit = (bounds.width / 100.0) as i32;
        let (time_step, rounded_earliest) = calculate_time_step(self.min, self.max, x_labels_can_fit);

        let labels = self.labels_cache.draw(renderer, bounds.size(), |frame| {
            frame.with_save(|frame| {
                let latest_in_millis = self.max * 1000; 
                let earliest_in_millis = self.min * 1000; 

                let mut time = rounded_earliest;
                let latest_time = NaiveDateTime::from_timestamp(self.max, 0);

                while time <= latest_time {
                    let time_in_millis = time.timestamp_millis();
                    
                    let x_position = ((time_in_millis - earliest_in_millis) as f64 / (latest_in_millis - earliest_in_millis) as f64) * bounds.width as f64;

                    if x_position >= 0.0 && x_position <= bounds.width as f64 {
                        let text_size = 12.0;
                        let label = canvas::Text {
                            content: time.format("%H:%M").to_string(),
                            position: Point::new(x_position as f32 - text_size, bounds.height as f32 - 20.0),
                            size: iced::Pixels(text_size),
                            color: Color::from_rgba8(200, 200, 200, 1.0),
                            ..canvas::Text::default()
                        };  

                        label.draw_with(|path, color| {
                            frame.fill(&path, color);
                        });
                    }
                    
                    time = time + time_step;
                }
            });
        });

        vec![labels]
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
    min: f32,
    max: f32,
}
impl canvas::Program<Message> for AxisLabelYCanvas<'_> {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
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

        let y_labels_can_fit = (bounds.height / 60.0) as i32;
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
                    let label = canvas::Text {
                        content: format!("{:.1}", y),
                        position: Point::new(5.0, y_position - text_size / 2.0),
                        size: iced::Pixels(text_size),
                        color: Color::from_rgba8(200, 200, 200, 1.0),
                        ..canvas::Text::default()
                    };  

                    label.draw_with(|path, color| {
                        frame.fill(&path, color);
                    });
                    y += step;
                }
            });
        });

        vec![labels]
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