mod ws_binance;

use chrono::{DateTime, Utc};
use iced::{
    executor, font, widget::{
        Row, button, canvas::{Cache, Frame, Geometry}, pick_list, shader::wgpu::hal::auxil::db, Column, Container, Text
    }, Alignment, Application, Command, Element, Event, Font, Length, Settings, Size, Subscription, Theme
};
use std::{ops::Sub, sync::Arc, time::Duration};
use plotters::prelude::ChartBuilder;
use plotters_backend::DrawingBackend;
use chrono::NaiveDateTime;
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
    WsToggle(),}

struct State {
    chart: Option<ExampleChart>,
    selected_ticker: Option<Ticker>,
    ws_state: WsState,
    ws_running: bool,
    trades: Vec<ws_binance::Trade>,
}

impl Application for State {
    type Message = self::Message;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            Self { 
                chart: None,
                selected_ticker: None,
                ws_state: WsState::Disconnected,
                ws_running: false,
                trades: Vec::new(),
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
                self.trades.clear();
                self.chart = Some(ExampleChart::new());
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
                ws_binance::Event::MessageReceived(trades_buffer) => {
                    if let Some(chart) = &mut self.chart {
                        chart.update(trades_buffer);
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

        let chart = match self.chart {
            Some(ref chart) => chart.view(),
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
            .push(chart);

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

struct ExampleChart {
    cache: Cache,
    data_points: VecDeque<(DateTime<Utc>, f32)>,
}

impl ExampleChart {
    fn new() -> Self {
        Self {
            cache: Cache::new(),
            data_points: VecDeque::new(),
        }
    }

    fn update(&mut self, mut trades_buffer: Vec<ws_binance::TradeWrapper>) {
        for ws_binance::TradeWrapper { stream: _, data } in trades_buffer.drain(..) {
            let time = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(data.time as i64 / 1000, 0), Utc);
            let price = data.price;
            self.data_points.push_back((time, price));
        }

        while self.data_points.len() > 6000 {
            self.data_points.pop_front();
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

impl Chart<Message> for ExampleChart {
    type State = ();
    // fn update(
    //     &mut self,
    //     event: Event,
    //     bounds: Rectangle,
    //     cursor: Cursor,
    // ) -> (event::Status, Option<Message>) {
    //     self.cache.clear();
    //     (event::Status::Ignored, None)
    // }

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

        const PLOT_LINE_COLOR: RGBColor = RGBColor(0, 175, 255);
        
        if self.data_points.len() > 1 {
            // x-axis range, acquire time range
            let newest_time = self
                .data_points
                .back()
                .unwrap()
                .0;
            let oldest_time = newest_time - chrono::Duration::seconds(30);
        
            // y-axis range, acquire price range within the time range
            let recent_data_points: Vec<_> = self.data_points.iter().filter_map(|&(time, price)| {
                if time >= oldest_time && time <= newest_time {
                    Some((time, price))
                } else {
                    None
                }
            }).collect();

            let mut y_min = f32::MAX;
            let mut y_max = f32::MIN;
            for (_, price) in &recent_data_points {
                y_min = y_min.min(*price);
                y_max = y_max.max(*price);
            }

            let mut chart = chart
                .x_label_area_size(0)
                .y_label_area_size(28)
                .margin(20)
                .build_cartesian_2d(oldest_time..newest_time, y_min..y_max)
                .expect("failed to build chart");

            chart
                .configure_mesh()
                .bold_line_style(GREY.mix(0.1))
                .light_line_style(GREY.mix(0.05))
                .axis_style(ShapeStyle::from(GREY.mix(0.45)).stroke_width(1))
                .y_labels(10)
                .y_label_style(
                    ("Noto Sans", 12)
                        .into_font()
                        .color(&GREY.mix(0.65))
                        .transform(FontTransform::Rotate90),
                )
                .y_label_formatter(&|y| format!("{}", y))
                .draw()
                .expect("failed to draw chart mesh");

            chart
                .draw_series(
                    AreaSeries::new(
                        recent_data_points,
                        0_f32,
                        PLOT_LINE_COLOR.mix(0.175),
                    )
                    .border_style(ShapeStyle::from(PLOT_LINE_COLOR).stroke_width(2)),
                )
            .expect("failed to draw chart data");
        }
    }
}