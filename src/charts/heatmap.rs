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
use std::collections::VecDeque;

use crate::market_data::Trade;
use crate::Message;

pub struct LineChart {
    cache: Cache,
    data_points: VecDeque<(DateTime<Utc>, f32, f32, bool)>,
    depth: VecDeque<(DateTime<Utc>, Vec<(f32, f32)>, Vec<(f32, f32)>)>,
}
impl LineChart {
    pub fn new() -> Self {
        Self {
            cache: Cache::new(),
            data_points: VecDeque::new(),
            depth: VecDeque::new(),
        }
    }

    pub fn update(&mut self, depth_update: u64, mut trades_buffer: Vec<Trade>, bids: Vec<(f32, f32)>, asks: Vec<(f32, f32)>) {
        let aggregate_time = 100; 
        let seconds = (depth_update / 1000) as i64;
        let nanoseconds = ((depth_update % 1000) / aggregate_time * aggregate_time * 1_000_000) as u32;
        let depth_update_time = match Utc.timestamp_opt(seconds, nanoseconds) {
            LocalResult::Single(dt) => dt,
            _ => return, 
        };

        for trade in trades_buffer.drain(..) {
            self.data_points.push_back((depth_update_time, trade.price, trade.qty, trade.is_sell));
        }
        if let Some((time, _, _)) = self.depth.back() {
            if *time == depth_update_time {
                self.depth.pop_back();
            }
        }
        self.depth.push_back((depth_update_time, bids, asks));

        while self.data_points.len() > 6000 {
            self.data_points.pop_front();
        }
        while self.depth.len() > 1000 {
            self.depth.pop_front();
        }

        self.cache.clear();
    }

    pub fn view(&self) -> Element<Message> {
        let chart = ChartWidget::new(self)
            .width(Length::Fill)
            .height(Length::Fill);

        chart.into()
    }
}
impl Chart<Message> for LineChart {
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
        
        if self.data_points.len() > 1 {
            // x-axis range, acquire time range
            let drawing_area;
            {
                let dummy_chart = chart
                    .build_cartesian_2d(0..1, 0..1) 
                    .expect("failed to build dummy chart");
                drawing_area = dummy_chart.plotting_area().dim_in_pixel();
            }
            let newest_time = self.depth.back().unwrap().0 + Duration::milliseconds(200);
            let oldest_time = newest_time - Duration::seconds(drawing_area.0 as i64 / 30);
        
            // y-axis range, acquire price range within the time range
            let mut y_min = f32::MAX;
            let mut y_max = f32::MIN;
            let recent_data_points: Vec<_> = self.data_points.iter().filter_map(|&(time, price, qty, bool)| {
                if time >= oldest_time && time <= newest_time {
                    Some((time, price, qty, bool))
                } else {
                    None
                }
            }).collect();

            let recent_depth: Vec<_> = self.depth.iter().filter_map(|(time, bids, asks)| {
                if time >= &oldest_time && time <= &newest_time {
                    if let Some((bid_price, _)) = bids.last() {
                        y_min = y_min.min(*bid_price);
                    } 
                    if let Some((ask_price, _)) = asks.last() {
                        y_max = y_max.max(*ask_price);
                    }
                    Some((time, bids, asks))
                } else {
                    None
                }
            }).collect();

            let mut chart = chart
                .x_label_area_size(20)
                .y_label_area_size(32)
                .margin(20)
                .build_cartesian_2d(oldest_time..newest_time, y_min..y_max)
                .expect("failed to build chart");

            chart
                .configure_mesh()
                .bold_line_style(GREY.mix(0.04))
                .light_line_style(GREY.mix(0.01))
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
                    x.format("%M:%S").to_string()
                })
                .draw()
                .expect("failed to draw chart mesh");

            let max_order_quantity = recent_depth.iter()
                .map(|(_, bids, asks)| {
                bids.iter().map(|(_, qty)| qty).chain(asks.iter().map(|(_, qty)| qty)).fold(f32::MIN, |current_max: f32, qty: &f32| f32::max(current_max, *qty))
            }).fold(f32::MIN, f32::max);
            for i in 0..20 { 
                let bids_i: Vec<(DateTime<Utc>, f32, f32)> = recent_depth.iter()
                    .map(|&(time, bid, _ask)| ((*time).clone(), bid[i].0, bid[i].1)).collect();
                let asks_i: Vec<(DateTime<Utc>, f32, f32)> = recent_depth.iter()
                    .map(|&(time, _bid, ask)| ((*time).clone(), ask[i].0, ask[i].1)).collect();
            
                chart
                    .draw_series(
                        bids_i.iter().map(|&(time, price, quantity)| {
                            let alpha = 0.1 + 0.9 * (quantity / max_order_quantity);
                            Pixel::new((time, price), RGBAColor(0, 144, 144, alpha.into()))
                        }),
                    )
                    .expect(&format!("failed to draw bids_{}", i));
            
                chart
                    .draw_series(
                        asks_i.iter().map(|&(time, price, quantity)| {
                            let alpha = 0.1 + 0.9 * (quantity / max_order_quantity);
                            Pixel::new((time, price), RGBAColor(192, 0, 192, alpha.into()))
                        }),
                    )
                    .expect(&format!("failed to draw asks_{}", i));
            }
            
            let (qty_min, qty_max) = recent_data_points.iter()
                .map(|&(_, _, qty, _)| qty)
                .fold((f32::MAX, f32::MIN), |(min, max), qty| (f32::min(min, qty), f32::max(max, qty)));
            chart
                .draw_series(
                    recent_data_points.iter().map(|&(time, price, qty, is_sell)| {
                        let radius = 1.0 + (qty - qty_min) * (35.0 - 1.0) / (qty_max - qty_min);
                        let color = if is_sell { RGBColor(192, 80, 77) } else { RGBColor(81, 205, 160)};
                        Circle::new(
                            (time, price), 
                            radius as i32,
                            ShapeStyle::from(color).filled(),
                        )
                    }),
                )
                .expect("failed to draw circles");
        }
    }
}
