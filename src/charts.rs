use iced::{
    widget::{canvas::Cache, button}, Border, Color, Point, Rectangle, Theme, Vector
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