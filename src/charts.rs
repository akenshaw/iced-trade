use chrono::NaiveDateTime;
use iced::{
    widget::{button, canvas::Cache}, Border, Color, Point, Rectangle, Theme, Vector
};
use iced::{
    mouse, widget::canvas, widget::canvas::{event::{self, Event}, stroke::Stroke, Geometry, Path}, Renderer, Size
};
pub mod heatmap;
pub mod footprint;
pub mod candlestick;
pub mod timeandsales;

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Translated(Vector),
    Scaled(f32, Option<Vector>),
    ChartBounds(Rectangle),
    AutoscaleToggle,
    CrosshairToggle,
    CrosshairMoved(Point),
    YScaling(f32, f32, bool),
    XScaling(f32, f32, bool),
}
struct CommonChartData {
    main_cache: Cache,

    mesh_cache: Cache,

    crosshair_cache: Cache,
    crosshair: bool,
    crosshair_position: Point,

    x_crosshair_cache: Cache,
    x_labels_cache: Cache,
    x_min_time: i64,
    x_max_time: i64,

    y_crosshair_cache: Cache,
    y_labels_cache: Cache,
    y_min_price: f32,
    y_max_price: f32,

    translation: Vector,
    scaling: f32,
    y_scaling: f32,
    autoscale: bool,

    bounds: Rectangle,
}
impl Default for CommonChartData {
    fn default() -> Self {
        CommonChartData {
            main_cache: Cache::default(),

            mesh_cache: Cache::default(),

            crosshair: true,
            crosshair_cache: Cache::default(),
            crosshair_position: Point::new(0.0, 0.0),

            x_crosshair_cache: Cache::default(),
            x_labels_cache: Cache::default(),
            x_min_time: 0,
            x_max_time: 0,

            y_crosshair_cache: Cache::default(),
            y_labels_cache: Cache::default(),
            y_min_price: 0.0,
            y_max_price: 0.0,

            translation: Vector::default(),
            scaling: 1.0,
            y_scaling: 1.0,
            autoscale: false,

            bounds: Rectangle::default(),
        }
    }
}

trait Chart {
    type DataPoint;

    fn get_common_data(&self) -> &CommonChartData;
    fn get_common_data_mut(&mut self) -> &mut CommonChartData;
}

#[derive(Debug, Clone, Copy)]
pub enum Interaction {
    None,
    Zoomin { last_position: Point },
    Panning { translation: Vector, start: Point },
}
impl Default for Interaction {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone)]
pub struct Region {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

fn chart_button(theme: &Theme, _status: button::Status, is_active: bool) -> button::Style {
    let palette = theme.extended_palette();

    button::Style {
        background: Some(Color::BLACK.into()),
        border: Border {
            color: {
                if is_active {
                    Color::from_rgba8(50, 50, 50, 1.0)
                } else {
                    Color::from_rgba8(20, 20, 20, 1.0)
                }
            },
            width: 1.0,
            radius: 2.0.into(),
        },
        text_color: palette.background.base.text,
        ..button::Style::default()
    }
}

// price steps, to be used for y-axis labels across all charts
const PRICE_STEPS: [f32; 15] = [
    1000.0,
    500.0,
    200.0,
    100.0,
    50.0,
    20.0,
    10.0,
    5.0,
    2.0,
    1.0,
    0.5,
    0.2,
    0.1,
    0.05,
    0.01,
];
fn calculate_price_step(highest: f32, lowest: f32, labels_can_fit: i32) -> (f32, f32) {
    let range = highest - lowest;
    let mut step = 1000.0; 

    for &s in PRICE_STEPS.iter().rev() {
        if range / s <= labels_can_fit as f32 {
            step = s;
            break;
        }
    }
    let rounded_lowest = (lowest / step).floor() * step;

    (step, rounded_lowest)
}

// time steps in ms, to be used for x-axis labels on candlesticks and footprint charts
const M1_TIME_STEPS: [i64; 9] = [
    1000 * 60 * 720, // 12 hour
    1000 * 60 * 180, // 3 hour
    1000 * 60 * 60, // 1 hour
    1000 * 60 * 30, // 30 min
    1000 * 60 * 15, // 15 min
    1000 * 60 * 10, // 10 min
    1000 * 60 * 5, // 5 min
    1000 * 60 * 2, // 2 min
    60 * 1000, // 1 min
];
const M3_TIME_STEPS: [i64; 9] = [
    1000 * 60 * 1440, // 24 hour
    1000 * 60 * 720, // 12 hour
    1000 * 60 * 180, // 6 hour
    1000 * 60 * 120, // 2 hour
    1000 * 60 * 60, // 1 hour
    1000 * 60 * 30, // 30 min
    1000 * 60 * 15, // 15 min
    1000 * 60 * 9, // 9 min
    1000 * 60 * 3, // 3 min
];
const M5_TIME_STEPS: [i64; 9] = [
    1000 * 60 * 1440, // 24 hour
    1000 * 60 * 720, // 12 hour
    1000 * 60 * 480, // 8 hour
    1000 * 60 * 240, // 4 hour
    1000 * 60 * 120, // 2 hour
    1000 * 60 * 60, // 1 hour
    1000 * 60 * 30, // 30 min
    1000 * 60 * 15, // 15 min
    1000 * 60 * 5, // 5 min
];

// time steps in ms, to be used for x-axis labels on heatmap chart
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

fn calculate_time_step(earliest: i64, latest: i64, labels_can_fit: i32, timeframe: Option<u16>) -> (i64, i64) {
    let duration = latest - earliest;

    if let Some(timeframe) = timeframe {
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

    } else {
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
}

pub struct AxisLabelXCanvas<'a> {
    labels_cache: &'a Cache,
    crosshair_cache: &'a Cache,
    crosshair_position: Point,
    crosshair: bool,
    min: i64,
    max: i64,
    timeframe: Option<u16>,
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
        if let Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
            *interaction = Interaction::None;
            return (event::Status::Ignored, None);
        }
    
        let Some(cursor_position) = cursor.position_in(bounds) else {
            return (event::Status::Ignored, None);
        };
    
        if !cursor.is_over(bounds) {
            return (event::Status::Ignored, None);
        }
    
        match event {
            Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::ButtonPressed(button) => {
                    if let mouse::Button::Left = button {
                        *interaction = Interaction::Zoomin {
                            last_position: cursor_position,
                        };
                        return (event::Status::Captured, None);
                    }
                }
                mouse::Event::CursorMoved { .. } => {
                    if let Interaction::Zoomin { ref mut last_position } = *interaction {
                        let difference_x = last_position.x - cursor_position.x;
    
                        if difference_x.abs() > 1.0 {
                            *last_position = cursor_position;
                            return (
                                event::Status::Captured,
                                Some(
                                    Message::XScaling(
                                        difference_x,
                                        {
                                            if let Some(cursor_to_center) = cursor.position_from(bounds.center()) {
                                                cursor_to_center.x
                                            } else {
                                                0.0
                                            }
                                        },
                                        false
                                    )
                                ),
                            );
                        }
                    }
                }
                mouse::Event::WheelScrolled { delta } => match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        return (
                            event::Status::Captured,
                            Some(
                                Message::XScaling(
                                    y,
                                    {
                                        if let Some(cursor_to_center) = cursor.position_from(bounds.center()) {
                                            cursor_to_center.x
                                        } else {
                                            0.0
                                        }
                                    },
                                    true
                                )
                            ),
                        );
                    }
                }
                _ => {}
            },
            _ => {}
        }
    
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

        let x_labels_can_fit = (bounds.width / 192.0) as i32;

        let (time_step, rounded_earliest) = calculate_time_step(earliest_in_millis, latest_in_millis, x_labels_can_fit, self.timeframe);
        
        let labels = self.labels_cache.draw(renderer, bounds.size(), |frame| {
            frame.with_save(|frame| {
                let mut time: i64 = rounded_earliest;

                while time <= latest_in_millis {                    
                    let x_position = ((time - earliest_in_millis) as f64 / (latest_in_millis - earliest_in_millis) as f64) * bounds.width as f64;

                    if x_position.is_nan() {
                        break;
                    }

                    if x_position >= 0.0 && x_position <= bounds.width as f64 {
                        let text_size = 12.0;
                        let time_as_datetime = NaiveDateTime::from_timestamp(time / 1000, 0);
                        
                        let time_format: &str;
                        if self.timeframe.is_some() {
                            time_format = "%H:%M";
                        } else {
                            time_format = "%M:%S";
                        }

                        let label = canvas::Text {
                            content: time_as_datetime.format(time_format).to_string(),
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
                let crosshair_millis = earliest_in_millis as f64 + crosshair_ratio * (latest_in_millis - earliest_in_millis) as f64;
        
                let (snap_ratio, text_content) = if let Some(timeframe) = self.timeframe {
                    let crosshair_time = NaiveDateTime::from_timestamp((crosshair_millis / 1000.0) as i64, 0);
                    let crosshair_timestamp = crosshair_time.timestamp();
                    let rounded_timestamp = (crosshair_timestamp as f64 / (timeframe as f64 * 60.0)).round() as i64 * timeframe as i64 * 60;
                    let rounded_time = NaiveDateTime::from_timestamp(rounded_timestamp, 0);
        
                    let snap_ratio = (rounded_timestamp as f64 * 1000.0 - earliest_in_millis as f64) / (latest_in_millis as f64 - earliest_in_millis as f64);
                    (snap_ratio, rounded_time.format("%H:%M").to_string())
                } else {
                    let crosshair_millis = (crosshair_millis / 100.0).round() * 100.0;
                    let crosshair_time = NaiveDateTime::from_timestamp((crosshair_millis / 1000.0).floor() as i64, ((crosshair_millis % 1000.0) * 1_000_000.0).round() as u32);
                    let crosshair_timestamp = crosshair_time.timestamp_millis();
        
                    let snap_ratio = (crosshair_timestamp as f64 - earliest_in_millis as f64) / (latest_in_millis as f64 - earliest_in_millis as f64);
                    (snap_ratio, crosshair_time.format("%M:%S:%3f").to_string().replace('.', ""))
                };
        
                let snap_x = snap_ratio * bounds.width as f64;

                if snap_x.is_nan() {
                    return;
                }
        
                let text_size = 12.0;
                let growth_amount = 6.0;
                let (rectangle_position, text_position) = if self.timeframe.is_some() {
                    (Point::new(snap_x as f32 - 14.0 - growth_amount, bounds.height - 20.0),
                     Point::new(snap_x as f32 - 14.0, bounds.height - 20.0))
                } else {
                    (Point::new(snap_x as f32 - 26.0 - growth_amount, bounds.height - 20.0),
                     Point::new(snap_x as f32 - 26.0, bounds.height - 20.0))
                };
        
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
            Interaction::Panning { .. } => mouse::Interaction::None,
            Interaction::Zoomin { .. } => mouse::Interaction::ResizingHorizontally,
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
    crosshair_position: Point,
    crosshair: bool,
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
        if let Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
            *interaction = Interaction::None;
            return (event::Status::Ignored, None);
        }
    
        let Some(cursor_position) = cursor.position_in(bounds) else {
            return (event::Status::Ignored, None);
        };
    
        if !cursor.is_over(bounds) {
            return (event::Status::Ignored, None);
        }
    
        match event {
            Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::ButtonPressed(button) => {
                    if let mouse::Button::Left = button {
                        *interaction = Interaction::Zoomin {
                            last_position: cursor_position,
                        };
                        return (event::Status::Captured, None);
                    }
                }
                mouse::Event::CursorMoved { .. } => {
                    if let Interaction::Zoomin { ref mut last_position } = *interaction {
                        let difference_y = last_position.y - cursor_position.y;
    
                        if difference_y.abs() > 1.0 {
                            *last_position = cursor_position;
                            return (
                                event::Status::Captured,
                                Some(
                                    Message::YScaling(
                                        difference_y,
                                        {
                                            if let Some(cursor_to_center) = cursor.position_from(bounds.center()) {
                                                cursor_to_center.y
                                            } else {
                                                0.0
                                            }
                                        },
                                        false
                                    )
                                ),
                            );
                        }
                    }
                }
                mouse::Event::WheelScrolled { delta } => match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        return (
                            event::Status::Captured,
                            Some(
                                Message::YScaling(
                                    y,
                                    {
                                        if let Some(cursor_to_center) = cursor.position_from(bounds.center()) {
                                            cursor_to_center.y
                                        } else {
                                            0.0
                                        }
                                    },
                                    true
                                )
                            ),
                        );
                    }
                }
                _ => {}
            },
            _ => {}
        }
    
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
                    let decimal_places = if step < 0.5 { 2 } else { usize::from(step < 1.0) };
                    let label_content = format!("{y:.decimal_places$}");
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
            });
        });
        let crosshair = self.y_croshair_cache.draw(renderer, bounds.size(), |frame| {
            if self.crosshair && self.crosshair_position.y > 0.0 {
                let text_size = 12.0;
                let y_range = self.max - self.min;
                let decimal_places = if step < 1.0 { 2 } else { 1 };
                let label_content = format!("{:.*}", decimal_places, self.min + (y_range * (candlesticks_area_height - self.crosshair_position.y) / candlesticks_area_height));
                
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
            Interaction::Zoomin { .. } => mouse::Interaction::ResizingVertically,
            Interaction::Panning { .. } => mouse::Interaction::None,
            Interaction::None if cursor.is_over(bounds) => {
                mouse::Interaction::ResizingVertically
            }
            Interaction::None => mouse::Interaction::default(),
        }
    }
}

// X-AXIS LABELS
pub struct AxisLabelsX<'a> {
    labels_cache: &'a Cache,
    crosshair_position: Point,
    crosshair: bool,
    min: i64,
    max: i64,
    scaling: f32,
    translation_x: f32,
    timeframe: u16,
    cell_width: f32,
}

impl AxisLabelsX<'_> {
    fn visible_region(&self, size: Size) -> Region {
        let width = size.width / self.scaling;
        let height = size.height / self.scaling;

        Region {
            x: -self.translation_x - width / 2.0,
            y: 0.0,
            width,
            height,
        }
    }

    fn x_to_time(&self, x: f32) -> i64 {
        let time_per_cell = self.timeframe as i64 * 60 * 1000;    
        self.max + ((x / self.cell_width) * time_per_cell as f32) as i64
    }
}

impl canvas::Program<Message> for AxisLabelsX<'_> {
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
            return (event::Status::Ignored, None);
        }
    
        let Some(cursor_position) = cursor.position_in(bounds) else {
            return (event::Status::Ignored, None);
        };
    
        if !cursor.is_over(bounds) {
            return (event::Status::Ignored, None);
        }
    
        match event {
            Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::ButtonPressed(button) => {
                    if let mouse::Button::Left = button {
                        *interaction = Interaction::Zoomin {
                            last_position: cursor_position,
                        };
                        return (event::Status::Captured, None);
                    }
                }
                mouse::Event::CursorMoved { .. } => {
                    if let Interaction::Zoomin { ref mut last_position } = *interaction {
                        let difference_x = last_position.x - cursor_position.x;
    
                        if difference_x.abs() > 1.0 {
                            *last_position = cursor_position;
                            return (
                                event::Status::Captured,
                                Some(
                                    Message::XScaling(
                                        difference_x,
                                        {
                                            if let Some(cursor_to_center) = cursor.position_from(bounds.center()) {
                                                cursor_to_center.x
                                            } else {
                                                0.0
                                            }
                                        },
                                        false
                                    )
                                ),
                            );
                        }
                    }
                }
                mouse::Event::WheelScrolled { delta } => match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        return (
                            event::Status::Captured,
                            Some(
                                Message::XScaling(
                                    y,
                                    {
                                        if let Some(cursor_to_center) = cursor.position_from(bounds.center()) {
                                            cursor_to_center.x
                                        } else {
                                            0.0
                                        }
                                    },
                                    true
                                )
                            ),
                        );
                    }
                }
                _ => {}
            },
            _ => {}
        }
    
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

        let x_labels_can_fit = (bounds.width / 192.0) as i32;
        
        let labels = self.labels_cache.draw(renderer, bounds.size(), |frame| {
            let region = self.visible_region(frame.size());

            let earliest_in_millis = self.x_to_time(region.x);
            let latest_in_millis = self.x_to_time(region.x + region.width);

            let (time_step, rounded_earliest) = calculate_time_step(earliest_in_millis, latest_in_millis, x_labels_can_fit, Some(self.timeframe));

            let mut time: i64 = rounded_earliest;

            while time <= latest_in_millis {                    
                let x_position = ((time - earliest_in_millis) as f64 / (latest_in_millis - earliest_in_millis) as f64) * bounds.width as f64;

                if x_position.is_nan() {
                    break;
                }

                if x_position >= 0.0 && x_position <= bounds.width as f64 {
                    let text_size = 12.0;
                    let time_as_datetime = NaiveDateTime::from_timestamp(time / 1000, 0);
                    
                    let time_format: &str = "%H:%M";

                    let label = canvas::Text {
                        content: time_as_datetime.format(time_format).to_string(),
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

            let line = Path::line(
                Point::new(0.0, bounds.height - 30.0), 
                Point::new(bounds.width, bounds.height - 30.0)
            );
            frame.stroke(&line, Stroke::default().with_color(Color::from_rgba8(81, 81, 81, 0.2)).with_width(1.0));

            if self.crosshair && self.crosshair_position.x > 0.0 {
                let crosshair_ratio = self.crosshair_position.x as f64 / bounds.width as f64;
                let crosshair_millis = earliest_in_millis as f64 + crosshair_ratio * (latest_in_millis - earliest_in_millis) as f64;
        
                let (snap_ratio, text_content) = {
                    let crosshair_time = NaiveDateTime::from_timestamp((crosshair_millis / 1000.0) as i64, 0);
                    let crosshair_timestamp = crosshair_time.timestamp();
                    let rounded_timestamp = (crosshair_timestamp as f64 / (self.timeframe as f64 * 60.0)).round() as i64 * self.timeframe as i64 * 60;
                    let rounded_time = NaiveDateTime::from_timestamp(rounded_timestamp, 0);
        
                    let snap_ratio = (rounded_timestamp as f64 * 1000.0 - earliest_in_millis as f64) / (latest_in_millis as f64 - earliest_in_millis as f64);
                    (snap_ratio, rounded_time.format("%H:%M").to_string())
                };
        
                let snap_x = snap_ratio * bounds.width as f64;

                if snap_x.is_nan() {
                    return;
                }
        
                let text_size = 12.0;
                let growth_amount = 6.0;
                let (rectangle_position, text_position) = {
                    (Point::new(snap_x as f32 - 14.0 - growth_amount, bounds.height - 20.0),
                        Point::new(snap_x as f32 - 14.0, bounds.height - 20.0))
                };
        
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
        
        vec![labels]
    }

    fn mouse_interaction(
        &self,
        interaction: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match interaction {
            Interaction::Panning { .. } => mouse::Interaction::None,
            Interaction::Zoomin { .. } => mouse::Interaction::ResizingHorizontally,
            Interaction::None if cursor.is_over(bounds) => {
                mouse::Interaction::ResizingHorizontally
            }
            Interaction::None => mouse::Interaction::default(),
        }
    }
}

// Y-AXIS LABELS
#[derive(Debug, Clone, Copy)]
pub struct AxisLabelsY<'a> {
    labels_cache: &'a Cache,
    translation_y: f32,
    scaling: f32,
    min: f32,
    max: f32,
    crosshair_position: Point,
    crosshair: bool,
    tick_size: f32,
    cell_height: f32,
}

impl AxisLabelsY<'_> {
    fn visible_region(&self, size: Size) -> Region {
        let width = size.width / self.scaling;
        let height = size.height / self.scaling;

        Region {
            x: 0.0,
            y: -self.translation_y - height / 2.0,
            width,
            height,
        }
    }

    fn y_to_price(&self, y: f32) -> f32 {
        self.min - (y / self.cell_height) * self.tick_size
    }
}

impl canvas::Program<Message> for AxisLabelsY<'_>{
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
            return (event::Status::Ignored, None);
        }
    
        let Some(cursor_position) = cursor.position_in(bounds) else {
            return (event::Status::Ignored, None);
        };
    
        if !cursor.is_over(bounds) {
            return (event::Status::Ignored, None);
        }
    
        match event {
            Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::ButtonPressed(button) => {
                    if let mouse::Button::Left = button {
                        *interaction = Interaction::Zoomin {
                            last_position: cursor_position,
                        };
                        return (event::Status::Captured, None);
                    }
                }
                mouse::Event::CursorMoved { .. } => {
                    if let Interaction::Zoomin { ref mut last_position } = *interaction {
                        let difference_y = last_position.y - cursor_position.y;
    
                        if difference_y.abs() > 1.0 {
                            *last_position = cursor_position;
                            return (
                                event::Status::Captured,
                                Some(
                                    Message::YScaling(
                                        difference_y,
                                        {
                                            if let Some(cursor_to_center) = cursor.position_from(bounds.center()) {
                                                cursor_to_center.y
                                            } else {
                                                0.0
                                            }
                                        },
                                        false
                                    )
                                ),
                            );
                        }
                    }
                }
                mouse::Event::WheelScrolled { delta } => match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        return (
                            event::Status::Captured,
                            Some(
                                Message::YScaling(
                                    y,
                                    {
                                        if let Some(cursor_to_center) = cursor.position_from(bounds.center()) {
                                            cursor_to_center.y
                                        } else {
                                            0.0
                                        }
                                    },
                                    true
                                )
                            ),
                        );
                    }
                }
                _ => {}
            },
            _ => {}
        }
    
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

        let labels = self.labels_cache.draw(renderer, bounds.size(), |frame| {
            let region = self.visible_region(frame.size());

            let highest = self.y_to_price(region.y);
            let lowest = self.y_to_price(region.y + region.height);

            let (step, rounded_lowest) = calculate_price_step(highest, lowest, y_labels_can_fit);
            let y_range = highest - lowest;
            
            let mut y = rounded_lowest;

            while y <= highest {
                let y_position = bounds.height - ((y - lowest) / y_range * bounds.height);

                let text_size = 12.0;
                let decimal_places = if step < 0.5 { 2 } else { usize::from(step < 1.0) };
                let label_content = format!("{y:.decimal_places$}");
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

            if self.crosshair && self.crosshair_position.y > 0.0 {
                let text_size = 12.0;
                let decimal_places = if step < 1.0 { 2 } else { 1 };
                let label_content = format!("{:.*}", decimal_places, lowest + (y_range * (bounds.height - self.crosshair_position.y) / bounds.height));
                
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

        vec![labels]
    }

    fn mouse_interaction(
        &self,
        interaction: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match interaction {
            Interaction::Zoomin { .. } => mouse::Interaction::ResizingVertically,
            Interaction::Panning { .. } => mouse::Interaction::None,
            Interaction::None if cursor.is_over(bounds) => {
                mouse::Interaction::ResizingVertically
            }
            Interaction::None => mouse::Interaction::default(),
        }
    }
}