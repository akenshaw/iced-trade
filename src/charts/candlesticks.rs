use chrono::{DateTime, Utc, Duration, TimeZone, LocalResult};
use iced::{
    widget::
        canvas::{Cache, Frame, Geometry}
    , Element, Length, Size
};
use plotters::prelude::ChartBuilder;
use plotters_backend::DrawingBackend;
use plotters_iced::{
    Chart, ChartWidget, Renderer as plottersRenderer,
};
use plotters::prelude::full_palette::GREY;
use std::collections::BTreeMap;

use crate::market_data::Kline;
use crate::Message;

pub struct CandlestickChart {
    cache: Cache,
    data_points: BTreeMap<DateTime<Utc>, (f32, f32, f32, f32)>,
    timeframe_in_minutes: i16,
}
impl CandlestickChart {
    pub fn new(klines: Vec<Kline>, timeframe_in_minutes: i16) -> Self {
        let mut data_points = BTreeMap::new();

        for kline in klines {
            let time = match Utc.timestamp_opt(kline.time as i64 / 1000, 0) {
                LocalResult::Single(dt) => dt,
                _ => continue, 
            };
            let open = kline.open;
            let high = kline.high;
            let low = kline.low;
            let close = kline.close;
            data_points.insert(time, (open, high, low, close));
        }

        Self {
            cache: Cache::new(),
            data_points,
            timeframe_in_minutes,
        }
    }

    pub fn update(&mut self, kline: Kline) {
        let time = match Utc.timestamp_opt(kline.time as i64 / 1000, 0) {
            LocalResult::Single(dt) => dt,
            _ => return,
        };
        let open = kline.open;
        let high = kline.high;
        let low = kline.low;
        let close = kline.close;
        self.data_points.insert(time, (open, high, low, close));

        self.cache.clear();
    }

    pub fn view(&self) -> Element<Message> {
        let chart = ChartWidget::new(self)
            .width(Length::Fill)
            .height(Length::Fill);

        chart.into()
    }
}
impl Chart<Message> for CandlestickChart {
    type State = ();
    #[inline]
    fn draw<R: plottersRenderer, F: Fn(&mut Frame)>(
        &self,
        renderer: &R,
        bounds: Size,
        draw_fn: F,
    ) -> Geometry {
        renderer.draw_cache(&self.cache, bounds, draw_fn)
    }

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut chart: ChartBuilder<DB>) {
        use plotters::prelude::*;

        let drawing_area;
        {
            let dummy_chart = chart
                .build_cartesian_2d(0..1, 0..1) 
                .expect("failed to build dummy chart");
            drawing_area = dummy_chart.plotting_area().dim_in_pixel();
        }
        let newest_time = *self.data_points.keys().last().unwrap_or(&Utc::now());
        let cutoff_number = ((drawing_area.0 as f64) / 12.0).round() as i64;
        let oldest_time = newest_time - Duration::minutes((cutoff_number*self.timeframe_in_minutes as i64).max(1));
        
        let visible_data_points: Vec<_> = self.data_points.iter().filter(|&(time, _)| {
            time >= &oldest_time && time <= &newest_time
        }).collect();

        let mut y_min = f32::MAX;
        let mut y_max = f32::MIN;
        for (_time, (_open, high, low, _close)) in &visible_data_points {
            y_min = y_min.min(*low);
            y_max = y_max.max(*high);
        }

        let mut chart = chart
            .x_label_area_size(20)
            .y_label_area_size(32)
            .margin(20)
            .build_cartesian_2d(oldest_time..newest_time, y_min..y_max)
            .expect("failed to build chart");

        chart
            .configure_mesh()
            .bold_line_style(GREY.mix(0.05))
            .light_line_style(GREY.mix(0.02))
            .axis_style(ShapeStyle::from(GREY.mix(0.45)).stroke_width(1))
            .y_labels(10)
            .y_label_style(
                ("Noto Sans", 12)
                    .into_font()
                    .color(&GREY.mix(0.65))
                    .transform(FontTransform::Rotate90),
            )
            .y_label_formatter(&|y| format!("{}", y))
            .x_labels(8) 
            .x_label_style(
                ("Noto Sans", 12)
                    .into_font()
                    .color(&GREY.mix(0.65))
            )
            .x_label_formatter(&|x| {
                x.format("%H:%M").to_string()
            })
            .draw()
            .expect("failed to draw chart mesh");

        chart.draw_series(
            visible_data_points.iter().map(|(time, (open, high, low, close))| {
                CandleStick::new(**time, *open, *high, *low, *close, RGBColor(81, 205, 160).filled(), RGBColor(192, 80, 77).filled(), 8)
            }),
        ).expect("failed to draw chart data");
    }
}
