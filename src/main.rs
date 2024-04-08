mod ws_binance;
use std::collections::HashMap;
use chrono::{DateTime, Utc, NaiveDateTime, Duration};
use iced::{
    executor, widget::{
        Row, button, canvas::{Cache, Frame, Geometry}, pick_list, shader::wgpu::hal::auxil::db, Column, Container, Text
    }, Alignment, Application, Command, Element, Event, Font, Length, Settings, Size, Subscription, Theme
};
use futures::TryFutureExt;
use plotters::prelude::ChartBuilder;
use plotters_backend::DrawingBackend;
use plotters_iced::{
    sample::lttb::{DataPoint, LttbSource},
    Chart, ChartWidget, Renderer,
};
use plotters::prelude::full_palette::GREY;
use std::{collections::VecDeque, time::Instant};
struct Wrapper<'a>(&'a DateTime<Utc>, &'a f32);
impl DataPoint for Wrapper<'_> {
    #[inline]
    fn x(&self) -> f64 {
        self.0.timestamp() as f64
    }
    #[inline]
    fn y(&self) -> f64 {
        *self.1 as f64
    }
}
impl std::fmt::Display for Ticker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Ticker::BTCUSDT => "BTCUSDT",
                Ticker::ETHUSDT => "ETHUSDT",
                Ticker::SOLUSDT => "SOLUSDT",
                Ticker::LTCUSDT => "LTCUSDT",
            }
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ticker {
    BTCUSDT,
    ETHUSDT,
    SOLUSDT,
    LTCUSDT,
}
impl Ticker {
    const ALL: [Ticker; 4] = [Ticker::BTCUSDT, Ticker::ETHUSDT, Ticker::SOLUSDT, Ticker::LTCUSDT];
}

enum WsState {
    Disconnected,
    Connected(ws_binance::Connection),
}
impl Default for WsState {
    fn default() -> Self {
        Self::Disconnected
    }
}

fn main() {
    State::run(Settings {
        antialiasing: true,
        default_font: Font::with_name("Noto Sans"),
        ..Settings::default()
    })
    .unwrap();
}

#[derive(Debug, Clone)]
enum Message {
    TickerSelected(Ticker),
    WsEvent(ws_binance::Event),
    WsToggle(),
    FetchEvent(Result<Vec<ws_binance::Kline>, std::string::String>),
}

struct State {
    trades_chart: Option<LineChart>,
    candlestick_chart: Option<CandlestickChart>,
    selected_ticker: Option<Ticker>,
    ws_state: WsState,
    ws_running: bool,
}

impl Application for State {
    type Message = self::Message;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            Self { 
                trades_chart: None,
                candlestick_chart: None,
                selected_ticker: None,
                ws_state: WsState::Disconnected,
                ws_running: false,
            },
           
            Command::batch([
                //Command::perform(tokio::task::spawn_blocking(generate_data), |data| {
                //    Message::DataLoaded(data.unwrap())
                //}),
            ]),
        )
    }

    fn title(&self) -> String {
        "Iced Trade".to_owned()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::TickerSelected(ticker) => {
                self.ws_running = false;
                self.selected_ticker = Some(ticker);
                Command::none()
            },
            Message::WsToggle() => {
                self.ws_running =! self.ws_running;
                dbg!(&self.ws_running);
                self.trades_chart = Some(LineChart::new());
                Command::perform(
                    ws_binance::fetch_klines(self.selected_ticker.unwrap().to_string())
                        .map_err(|err| format!("{}", err)), 
                    |klines| {
                        Message::FetchEvent(klines)
                    }
                )
            },
            Message::FetchEvent(klines) => {
                match klines {
                    Ok(klines) => {
                        self.candlestick_chart = Some(CandlestickChart::new(klines));
                    },
                    Err(err) => {
                        eprintln!("Error fetching klines: {}", err);
                        self.candlestick_chart = Some(CandlestickChart::new(vec![]));
                    },
                }
                Command::none()
            },
            Message::WsEvent(event) => match event {
                ws_binance::Event::Connected(connection) => {
                    self.ws_state = WsState::Connected(connection);
                    Command::none()
                }
                ws_binance::Event::Disconnected => {
                    self.ws_state = WsState::Disconnected;
                    Command::none()
                }
                ws_binance::Event::DepthReceived(depth_update, bids, asks, trades_buffer) => {
                    if let Some(chart) = &mut self.trades_chart {
                        chart.update(depth_update, trades_buffer, bids, asks);
                    }
                    Command::none()
                }
                ws_binance::Event::KlineReceived(kline) => {
                    if let Some(chart) = &mut self.candlestick_chart {
                        chart.update(kline);
                    }
                    Command::none()
                }
            }, 
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let button_text = if self.ws_running { "Disconnect" } else { "Connect" };
        let ws_button = button(button_text).on_press(Message::WsToggle());

        let pick_list = pick_list(
            &Ticker::ALL[..],
            self.selected_ticker,
            Message::TickerSelected,
        )
        .placeholder("Choose a ticker...");

        let trades_chart = match self.trades_chart {
            Some(ref trades_chart) => trades_chart.view(),
            None => Text::new("").into(),
        };
        let candlestick_chart = match self.candlestick_chart {
            Some(ref candlestick_chart) => candlestick_chart.view(),
            None => Text::new("Loading...").into(),
        };

        let controls = Row::new()
            .spacing(20)
            .align_items(Alignment::Center)
            .push(pick_list)
            .push(ws_button);

        let content = Column::new()
            .spacing(20)
            .align_items(Alignment::Start)
            .width(Length::Fill)
            .height(Length::Fill)
            .push(controls)
            .push(trades_chart)
            .push(candlestick_chart);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(20)
            .center_x()
            .center_y()
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        match (&self.selected_ticker, self.ws_running) {
            (Some(selected_ticker), true) => ws_binance::connect(selected_ticker.to_string()).map(Message::WsEvent),
            _ => Subscription::none(),
        }
    }

    fn theme(&self) -> Self::Theme {
        Theme::Oxocarbon
    }
}

struct CandlestickChart {
    cache: Cache,
    data_points: HashMap<DateTime<Utc>, (f32, f32, f32, f32)>,
}
impl CandlestickChart {
    fn new(klines: Vec<ws_binance::Kline>) -> Self {
        let mut data_points = HashMap::new();

        for kline in klines {
            let time = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(kline.time as i64 / 1000, 0), Utc);
            let open = kline.open;
            let high = kline.high;
            let low = kline.low;
            let close = kline.close;
            data_points.insert(time, (open, high, low, close));
        }

        Self {
            cache: Cache::new(),
            data_points,
        }
    }

    fn update(&mut self, kline: ws_binance::Kline) {
        let time = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(kline.time as i64 / 1000, 0), Utc);
        let open = kline.open;
        let high = kline.high;
        let low = kline.low;
        let close = kline.close;
        self.data_points.insert(time, (open, high, low, close));

        self.cache.clear();
    }

    fn view(&self) -> Element<Message> {
        let chart = ChartWidget::new(self)
            .width(Length::Fill)
            .height(Length::Fill);

        chart.into()
    }
}
impl Chart<Message> for CandlestickChart {
    type State = ();
    #[inline]
    fn draw<R: Renderer, F: Fn(&mut Frame)>(
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
        let newest_time = *self.data_points.keys().max().unwrap_or(&Utc::now());
        let oldest_time = newest_time - Duration::minutes(drawing_area.0 as i64 / 20);

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
            .x_label_area_size(28)
            .y_label_area_size(28)
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
                CandleStick::new(**time, *open, *high, *low, *close, RGBColor(81, 205, 160).filled(), RGBColor(192, 80, 77).filled(), 15)
            }),
        ).expect("failed to draw chart data");
    }
}

struct LineChart {
    cache: Cache,
    data_points: VecDeque<(DateTime<Utc>, f32, f32, bool)>,
    depth: VecDeque<(DateTime<Utc>, Vec<(f32, f32)>, Vec<(f32, f32)>)>,
}
impl LineChart {
    fn new() -> Self {
        Self {
            cache: Cache::new(),
            data_points: VecDeque::new(),
            depth: VecDeque::new(),
        }
    }

    fn update(&mut self, depth_update: u64, mut trades_buffer: Vec<ws_binance::Trade>, bids: Vec<(f32, f32)>, asks: Vec<(f32, f32)>) {
        let aggregate_time = 100; 
        let depth_update_time = DateTime::<Utc>::from_utc(
            NaiveDateTime::from_timestamp(
                (depth_update / 1000) as i64, 
                ((depth_update % 1000) / aggregate_time * aggregate_time * 1_000_000) as u32
            ), 
            Utc
        );

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

    fn view(&self) -> Element<Message> {
        let chart = ChartWidget::new(self)
            .width(Length::Fill)
            .height(Length::Fill);

        chart.into()
    }
}
impl Chart<Message> for LineChart {
    type State = ();
    #[inline]
    fn draw<R: Renderer, F: Fn(&mut Frame)>(
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
                .x_label_area_size(28)
                .y_label_area_size(28)
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

            for i in 0..20 { 
                let bids_i: Vec<(DateTime<Utc>, f32, f32)> = recent_depth.iter().map(|&(time, bid, _ask)| ((*time).clone(), bid[i].0, bid[i].1)).collect();
                let asks_i: Vec<(DateTime<Utc>, f32, f32)> = recent_depth.iter().map(|&(time, _bid, ask)| ((*time).clone(), ask[i].0, ask[i].1)).collect();
            
                let max_order_quantity = bids_i.iter()
                    .map(|&(_time, _price, quantity)| quantity)
                    .chain(asks_i.iter().map(|&(_time, _price, quantity)| quantity))
                    .fold(f32::MIN, f32::max);

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
            
            let qty_min = recent_data_points.iter().map(|&(_, _, qty, _)| qty).fold(f32::MAX, f32::min);
            let qty_max = recent_data_points.iter().map(|&(_, _, qty, _)| qty).fold(f32::MIN, f32::max);
            chart
                .draw_series(
                    recent_data_points.iter().map(|&(time, price, qty, is_sell)| {
                        let radius = 1.0 + (qty - qty_min) * (30.0 - 1.0) / (qty_max - qty_min);
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