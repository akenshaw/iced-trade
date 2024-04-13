mod ws_binance;
use std::collections::BTreeMap;
use chrono::{DateTime, Utc, Duration, TimeZone, LocalResult};
use iced::{
    executor, Color, widget::{
        button, canvas::{path::lyon_path::geom::euclid::num::Round, Cache, Frame, Geometry}, pick_list, Column, Container, Row, Text
    }, Alignment, Application, Command, Element, Font, Length, Settings, Size, Subscription, Theme
};
use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    column, container, row, scrollable, text, responsive
};
use futures::TryFutureExt;
use plotters::prelude::ChartBuilder;
use plotters_backend::DrawingBackend;
use plotters_iced::{
    sample::lttb::DataPoint,
    Chart, ChartWidget, Renderer,
};
use plotters::prelude::full_palette::GREY;
use std::collections::VecDeque;
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

#[derive(Clone, Copy)]
struct Pane {
    id: usize,
    pub is_pinned: bool,
}
impl Pane {
    fn new(id: usize) -> Self {
        Self {
            id,
            is_pinned: false,
        }
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
    TimeframeSelected(&'static str),
    WsEvent(ws_binance::Event),
    WsToggle(),
    FetchEvent(Result<Vec<ws_binance::Kline>, std::string::String>),
    Split(pane_grid::Axis, pane_grid::Pane),
    Clicked(pane_grid::Pane),
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    TogglePin(pane_grid::Pane),
    Maximize(pane_grid::Pane),
    Restore,
    Close(pane_grid::Pane),
    CloseFocused,
}

struct State {
    trades_chart: Option<LineChart>,
    candlestick_chart: Option<CandlestickChart>,
    selected_ticker: Option<Ticker>,
    selected_timeframe: Option<&'static str>,
    ws_state: WsState,
    ws_running: bool,
    panes: pane_grid::State<Pane>,
    panes_created: usize,
    focus: Option<pane_grid::Pane>,
    first_pane: pane_grid::Pane,
}

impl Application for State {
    type Message = self::Message;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let (panes, first_pane) = pane_grid::State::new(Pane::new(0));
        (
            Self { 
                trades_chart: None,
                candlestick_chart: None,
                selected_ticker: None,
                selected_timeframe: Some("1m"),
                ws_state: WsState::Disconnected,
                ws_running: false,
                panes,
                panes_created: 1,
                focus: None,
                first_pane,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "Iced Trade".to_owned()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::TickerSelected(ticker) => {
                self.selected_ticker = Some(ticker);
                Command::none()
            },
            Message::TimeframeSelected(timeframe) => {
                self.selected_timeframe = Some(timeframe);
                Command::none()
            },
            Message::WsToggle() => {
                self.ws_running =! self.ws_running;
                dbg!(&self.ws_running);
                if self.ws_running {
                    self.trades_chart = Some(LineChart::new());
                    let fetch_klines = Command::perform(
                        ws_binance::fetch_klines(self.selected_ticker.unwrap().to_string(), self.selected_timeframe.unwrap().to_string())
                            .map_err(|err| format!("{}", err)), 
                        |klines| {
                            Message::FetchEvent(klines)
                        }
                    );
                    if self.panes.len() == 1 {
                        let first_pane = self.first_pane;
                        let split_pane = Command::perform(
                            async move {
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                (pane_grid::Axis::Horizontal, first_pane) 
                            },
                            |(axis, pane)| {
                                Message::Split(axis, pane)
                            }
                        );
                        Command::batch(vec![fetch_klines, split_pane])
                    } else {
                        fetch_klines
                    }
                } else {
                    self.trades_chart = None;
                    self.candlestick_chart = None;
                    Command::none()
                }
            },       
            Message::FetchEvent(klines) => {
                match klines {
                    Ok(klines) => {
                        let timeframe_in_minutes = match self.selected_timeframe.unwrap() {
                            "1m" => 1,
                            "3m" => 3,
                            "5m" => 5,
                            "15m" => 15,
                            "30m" => 30,
                            _ => 1,
                        };
                        self.candlestick_chart = Some(CandlestickChart::new(klines, timeframe_in_minutes));
                    },
                    Err(err) => {
                        eprintln!("Error fetching klines: {}", err);
                        self.candlestick_chart = Some(CandlestickChart::new(vec![], 1));
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
            Message::Split(axis, pane) => {
                let result =
                    self.panes.split(axis, pane, Pane::new(self.panes_created));

                if let Some((pane, _)) = result {
                    self.focus = Some(pane);
                }

                self.panes_created += 1;
                Command::none()
            }
            Message::Clicked(pane) => {
                self.focus = Some(pane);
                Command::none()
            }
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Command::none()
            }
            Message::Dragged(pane_grid::DragEvent::Dropped {
                pane,
                target,
            }) => {
                self.panes.drop(pane, target);
                Command::none()
            }
            Message::Dragged(_) => {
                Command::none()
            }
            Message::TogglePin(pane) => {
                if let Some(Pane { is_pinned, .. }) = self.panes.get_mut(pane) {
                    *is_pinned = !*is_pinned;
                }
                Command::none()
            }
            Message::Maximize(pane) => {
                self.panes.maximize(pane);
                Command::none()
            },
            Message::Restore => {
                self.panes.restore();
                Command::none()
            }
            Message::Close(pane) => {
                if let Some((_, sibling)) = self.panes.close(pane) {
                    self.focus = Some(sibling);
                }
                Command::none()
            }
            Message::CloseFocused => {
                if let Some(pane) = self.focus {
                    if let Some(Pane { is_pinned, .. }) = self.panes.get(pane) {
                        if !is_pinned {
                            if let Some((_, sibling)) = self.panes.close(pane) {
                                self.focus = Some(sibling);
                            }
                        }
                    }
                }
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let focus = self.focus;
        let total_panes = self.panes.len();

        let pane_grid = PaneGrid::new(&self.panes, |id, pane, is_maximized| {
            let is_focused = focus == Some(id);
    
            let title = if pane.id == 0 {
                "Heatmap"
            } else {
                "Candlesticks"
            };
    
            let mut content = pane_grid::Content::new(responsive(move |size| {
                view_content(id, total_panes, pane.is_pinned, size, pane.id.to_string(), &self.trades_chart, &self.candlestick_chart)
            }))
            .style(if is_focused {
                style::pane_focused
            } else {
                style::pane_active
            });
    
            if is_focused {
                let title_bar = pane_grid::TitleBar::new(title)
                    .controls(view_controls(
                        id,
                        total_panes,
                        pane.is_pinned,
                        is_maximized,
                    ))
                    .padding(5)
                    .style(style::title_bar_focused);
    
                content = content.title_bar(title_bar);
            }
    
            content
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(10)
        .on_click(Message::Clicked)
        .on_drag(Message::Dragged)
        .on_resize(10, Message::Resized);

        let button_text = if self.ws_running { "Disconnect" } else { "Connect" };
        let ws_button = button(button_text).on_press(Message::WsToggle());

        let mut ws_controls = Row::new()
            .spacing(20)
            .align_items(Alignment::Center)
            .push(ws_button);

            if !self.ws_running {
                let symbol_pick_list = pick_list(
                    &Ticker::ALL[..],
                    self.selected_ticker,
                    Message::TickerSelected,
                )
                .placeholder("Choose a ticker...");
            
                let timeframe_pick_list = pick_list(
                    &["1m", "3m", "5m", "15m", "30m"][..],
                    self.selected_timeframe,
                    Message::TimeframeSelected,
                );
            
                ws_controls = ws_controls.push(timeframe_pick_list)
                    .push(symbol_pick_list);
            } else {
                ws_controls = ws_controls.push(Text::new(self.selected_ticker.unwrap().to_string()).size(20));
            }

        let content = Column::new()
            .spacing(5)
            .align_items(Alignment::Start)
            .width(Length::Fill)
            .height(Length::Fill)
            .push(ws_controls)
            .push(pane_grid);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(10)
            .center_x()
            .center_y()
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        match (&self.selected_ticker, self.ws_running) {
            (Some(selected_ticker), true) => ws_binance::connect(selected_ticker.to_string(), self.selected_timeframe.unwrap().to_string()).map(Message::WsEvent),
            _ => Subscription::none(),
        }
    }

    fn theme(&self) -> Self::Theme {
        Theme::Oxocarbon
    }
}

fn view_content<'a>(
    _pane: pane_grid::Pane,
    _total_panes: usize,
    _is_pinned: bool,
    _size: Size,
    pane_id: String,
    trades_chart: &'a Option<LineChart>,
    candlestick_chart: &'a Option<CandlestickChart>,
) -> Element<'a, Message> {

    let chart = match pane_id.as_str() {
        "0" => trades_chart.as_ref().map(LineChart::view).unwrap_or_else(|| Text::new("No data").into()),
        "1" => candlestick_chart.as_ref().map(CandlestickChart::view).unwrap_or_else(|| Text::new("No data").into()),
        _ => Text::new("No data").into(),
    };

    container(chart)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_controls<'a>(
    pane: pane_grid::Pane,
    total_panes: usize,
    _is_pinned: bool,
    is_maximized: bool,
) -> Element<'a, Message> {
    let mut row = row![].spacing(5);

    if total_panes > 1 {
        let buttons = if is_maximized {
            vec![
                ("Restore", Message::Restore),
                //("Split Horizontally", Message::Split(pane_grid::Axis::Horizontal, pane)),
                //("Split Vertically", Message::Split(pane_grid::Axis::Vertical, pane))
            ]
        } else {
            vec![
                ("Maximize", Message::Maximize(pane)),
                //("Split Horizontally", Message::Split(pane_grid::Axis::Horizontal, pane)),
                //("Split Vertically", Message::Split(pane_grid::Axis::Vertical, pane))
            ]
        };

        for (content, message) in buttons {
            row = row.push(
                button(text(content).size(14))
                    .padding(3)
                    .on_press(message),
            );
        }
    }

    //let close = button(text("Close").size(14))
    //    .padding(3)
    //    .on_press_maybe(if total_panes > 1 && !is_pinned {
    //        Some(Message::Close(pane))
    //    } else {
    //        None
    //    });
    //row.push(close).into()
    row.into()
}

mod style {
    use iced::widget::container;
    use iced::{Border, Theme};

    pub fn title_bar_active(theme: &Theme) -> container::Appearance {
        let palette = theme.extended_palette();

        container::Appearance {
            text_color: Some(palette.background.strong.text),
            background: Some(palette.background.strong.color.into()),
            ..Default::default()
        }
    }
    pub fn title_bar_focused(theme: &Theme) -> container::Appearance {
        let palette = theme.extended_palette();

        container::Appearance {
            text_color: Some(palette.primary.strong.text),
            background: Some(palette.primary.strong.color.into()),
            ..Default::default()
        }
    }
    pub fn pane_active(theme: &Theme) -> container::Appearance {
        let palette = theme.extended_palette();

        container::Appearance {
            //background: Some(palette.background.weak.color.into()),
            border: Border {
                width: 2.0,
                color: palette.background.strong.color,
                ..Border::default()
            },
            ..Default::default()
        }
    }
    pub fn pane_focused(theme: &Theme) -> container::Appearance {
        let palette = theme.extended_palette();

        container::Appearance {
            //background: Some(palette.background.weak.color.into()),
            border: Border {
                width: 2.0,
                color: palette.primary.strong.color,
                ..Border::default()
            },
            ..Default::default()
        }
    }
}

struct CandlestickChart {
    cache: Cache,
    data_points: BTreeMap<DateTime<Utc>, (f32, f32, f32, f32)>,
    timeframe_in_minutes: i16,
}
impl CandlestickChart {
    fn new(klines: Vec<ws_binance::Kline>, timeframe_in_minutes: i16) -> Self {
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

    fn update(&mut self, kline: ws_binance::Kline) {
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
        let newest_time = *self.data_points.keys().last().unwrap_or(&Utc::now());
        let cutoff_number = (drawing_area.0 as i64 / 12).round();
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
                CandleStick::new(**time, *open, *high, *low, *close, RGBColor(81, 205, 160).filled(), RGBColor(192, 80, 77).filled(), 8)
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