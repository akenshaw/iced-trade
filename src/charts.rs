use iced::{
    widget::{canvas::Cache, button}, Border, Color, Point, Rectangle, Theme, Vector
};

pub mod heatmap;
pub mod footprint;
pub mod candlesticks;

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