use chrono::NaiveDateTime;
use iced::{
    widget::{canvas::Cache, button}, Border, Color, Point, Rectangle, Theme, Vector
};
use iced::{
    mouse, widget::canvas, widget::canvas::{event::{self, Event}, stroke::Stroke, Geometry, Path}, Renderer, Size
};

pub mod heatmap;
pub mod footprint;
pub mod candlestick;

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Translated(Vector),
    Scaled(f32, Option<Vector>),
    ChartBounds(Rectangle),
    AutoscaleToggle,
    CrosshairToggle,
    CrosshairMoved(Point),
    YScaling(f32),
}
struct CommonChartData {
    main_cache: Cache,

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
    autoscale: bool,

    bounds: Rectangle,
}
impl Default for CommonChartData {
    fn default() -> Self {
        CommonChartData {
            main_cache: Cache::default(),

            crosshair: false,
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
            autoscale: true,

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
    Drawing,
    Erasing,
    Panning { translation: Vector, start: Point },
}
impl Default for Interaction {
    fn default() -> Self {
        Self::None
    }
}

fn chart_button(_theme: &Theme, _status: button::Status, is_active: bool) -> button::Style {
    button::Style {
        background: Some(Color::from_rgba8(20, 20, 20, 1.0).into()),
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
        text_color: Color::WHITE,
        ..button::Style::default()
    }
}

// price steps, to be used for y-axis labels on all charts
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
fn calculate_time_step(earliest: i64, latest: i64, labels_can_fit: i32, timeframe: u16) -> (i64, i64) {
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
    timeframe: u16,
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
        let (time_step, rounded_earliest) = calculate_time_step(self.min, self.max, x_labels_can_fit, self.timeframe);

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
                let crosshair_time = NaiveDateTime::from_timestamp((crosshair_millis / 1000.0) as i64, 0);

                let crosshair_timestamp = crosshair_time.timestamp();
                let rounded_timestamp = (crosshair_timestamp as f64 / (self.timeframe as f64 * 60.0)).round() as i64 * self.timeframe as i64 * 60;
                let rounded_time = NaiveDateTime::from_timestamp(rounded_timestamp, 0);

                let snap_ratio = (rounded_timestamp as f64 * 1000.0 - earliest_in_millis as f64) / (latest_in_millis as f64 - earliest_in_millis as f64);
                let snap_x = snap_ratio * bounds.width as f64;

                let text_size = 12.0;
                let text_content = rounded_time.format("%H:%M").to_string();
                let growth_amount = 6.0; 
                let rectangle_position = Point::new(snap_x as f32 - 14.0 - growth_amount, bounds.height - 20.0);
                let text_position = Point::new(snap_x as f32 - 14.0, bounds.height - 20.0);

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