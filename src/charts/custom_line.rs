use std::collections::BTreeMap;
use chrono::{DateTime, Utc, TimeZone, LocalResult};
use iced::mouse;
use iced::widget::canvas;
use iced::widget::canvas::event::{self, Event};
use iced::widget::canvas::stroke::Stroke;
use iced::widget::canvas::{Cache, Geometry, Path, Canvas};
use iced::window;
use iced::{
    Color, Point, Rectangle, Renderer, Size,
    Theme, Vector, Element, Length
};
use crate::market_data::Kline;

#[derive(Debug, Clone)]
pub enum Message {
    Translated(Vector),
    Scaled(f32, Option<Vector>),
}

#[derive(Debug)]
pub struct CustomLine {
    space_cache: Cache,
    system_cache: Cache,
    translation: Vector,
    scaling: f32,
    klines_raw: BTreeMap<DateTime<Utc>, (f32, f32, f32, f32)>,
    autoscale: bool,
}
impl CustomLine {
    const MIN_SCALING: f32 = 0.1;
    const MAX_SCALING: f32 = 2.0;

    pub fn new(_klines: Vec<Kline>, _timeframe_in_minutes: i16) -> CustomLine {
        let _size = window::Settings::default().size;
        CustomLine {
            space_cache: canvas::Cache::default(),
            system_cache: canvas::Cache::default(),
            klines_raw: BTreeMap::new(),
            translation: Vector::default(),
            scaling: 1.0,
            autoscale: true,
        }
    }

    pub fn set_dataset(&mut self, klines: Vec<Kline>) {
        self.klines_raw.clear();

        for kline in klines {
            let time = match Utc.timestamp_opt(kline.time as i64 / 1000, 0) {
                LocalResult::Single(dt) => dt,
                _ => continue, 
            };
            let open = kline.open;
            let high = kline.high;
            let low = kline.low;
            let close = kline.close;
            self.klines_raw.insert(time, (open, high, low, close));
        }

        self.system_cache.clear();
    }

    pub fn insert_datapoint(&mut self, kline: Kline) {
        let time = match Utc.timestamp_opt(kline.time as i64 / 1000, 0) {
            LocalResult::Single(dt) => dt,
            _ => return, 
        };
        let open = kline.open;
        let high = kline.high;
        let low = kline.low;
        let close = kline.close;
        self.klines_raw.insert(time, (open, high, low, close));

        self.system_cache.clear();
    }
    
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Translated(translation) => {
                if self.autoscale {
                    self.translation.x = translation.x;
                } else {
                    self.translation = translation;
                }

                self.system_cache.clear();
                self.space_cache.clear();
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

                self.system_cache.clear();
                self.space_cache.clear();
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
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
        let earliest: i64 = latest - (6400.0 / self.scaling) as i64;
    
        let (visible_klines, highest, lowest, avg_body_height) = self.klines_raw.iter()
            .filter(|(time, _)| {
                let timestamp = time.timestamp();
                timestamp >= earliest && timestamp <= latest
            })
            .fold((vec![], f32::MIN, f32::MAX, 0.0), |(mut klines, highest, lowest, total_body_height), (time, (open, high, low, close))| {
                let body_height = (open - close).abs();
                klines.push((*time, (*open, *high, *low, *close)));
                (klines, highest.max(*high), lowest.min(*low), total_body_height + body_height)
            });
    
        if visible_klines.is_empty() {
            return vec![];
        }
    
        let avg_body_height = avg_body_height / visible_klines.len() as f32;
        let (highest, lowest) = (highest + avg_body_height, lowest - avg_body_height);
        let y_range = highest - lowest;

        let background = 
            self.space_cache.draw(renderer, bounds.size(), |frame| {
                let text_size = 14.0;
                let highest_label = canvas::Text {
                    content: format!("{:.2}", highest),
                    position: Point::new(0.0, 0.0),
                    size: iced::Pixels(text_size),
                    color: Color::WHITE,
                    ..canvas::Text::default()
                };            
                let lowest_label = canvas::Text {
                    content: format!("{:.2}", lowest),
                    position: Point::new(0.0, bounds.height - (text_size + text_size / 2.0)),
                    size: iced::Pixels(text_size),
                    color: Color::WHITE,
                    ..canvas::Text::default()
                };
                highest_label.draw_with(|path, color| {
                    frame.fill(&path, color);
                });
                lowest_label.draw_with(|path, color| {
                    frame.fill(&path, color);
                });
            });

        let candlesticks = 
            self.system_cache.draw(renderer, bounds.size(), |frame| {
                frame.with_save(|frame| {
                    for (time, (open, high, low, close)) in visible_klines {
                        let x_position: f64 = ((time.timestamp() - earliest) as f64 / (latest - earliest) as f64) * bounds.width as f64;
                        
                        let y_open = bounds.height - ((open - lowest) / y_range * bounds.height);
                        let y_high = bounds.height - ((high - lowest) / y_range * bounds.height);
                        let y_low = bounds.height - ((low - lowest) / y_range * bounds.height);
                        let y_close = bounds.height - ((close - lowest) / y_range * bounds.height);
                        
                        let color = if close > open { Color::from_rgb8(81, 205, 160) } else { Color::from_rgb8(192, 80, 77) };
    
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
                    }
                });
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

impl Default for CustomLine {
    fn default() -> Self {
        Self::new(vec![], 1)
    }
}