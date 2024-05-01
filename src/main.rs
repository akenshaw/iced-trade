mod data_providers;
use data_providers::binance::{user_data, market_data};
mod charts;
use charts::{heatmap, candlesticks};

use crate::heatmap::LineChart;
use crate::candlesticks::CandlestickChart;

use std::cell::RefCell;
use chrono::{DateTime, Utc};
use iced::{
    font, executor, widget::{
        button, pick_list, text_input, Column, Container, Row, Space, Text, horizontal_space
    }, window::icon, Alignment, Application, Command, Element, Length, Renderer, Settings, Size, Subscription, Theme, Font, alignment, Border
};
use iced::widget::{column as col, vertical_space};

use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    column, container, row, scrollable, text, responsive
};
use iced_table::table;
use futures::TryFutureExt;
use plotters_iced::sample::lttb::DataPoint;

use iced_aw::menu::{self, Item, Menu, StyleSheet};
use iced_aw::style::MenuBarStyle;
use iced_aw::{menu_bar, menu_items};

use std::collections::HashMap;

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
pub enum Ticker {
    BTCUSDT,
    ETHUSDT,
    SOLUSDT,
    LTCUSDT,
}
impl Ticker {
    const ALL: [Ticker; 4] = [Ticker::BTCUSDT, Ticker::ETHUSDT, Ticker::SOLUSDT, Ticker::LTCUSDT];
}

// binance testnet api keys
const API_KEY: &str = "d5811ebf135cc577a5d657216adaafb0b8631cdc85d6a1122f04438ffdb17af1";
const SECRET_KEY: &str = "fd4b4e3286245d1eb6eda3c4538b52a3159dd35a3647ea8744a5f1d7d83a3bb5";

const ICON_BYTES: &[u8] = include_bytes!("fonts/icons.ttf");
const ICON: Font = Font::with_name("icons");

enum Icon {
    Locked,
    Unlocked,
    ResizeFull,
    ResizeSmall,
    Close,
    Add
}

impl From<Icon> for char {
    fn from(icon: Icon) -> Self {
        match icon {
            Icon::Unlocked => '\u{E800}',
            Icon::Locked => '\u{E801}',
            Icon::ResizeFull => '\u{E802}',
            Icon::ResizeSmall => '\u{E803}',
            Icon::Close => '\u{E804}',
            Icon::Add => '\u{F0FE}',
        }
    }
}

enum WsState {
    Connected(market_data::Connection),
    Disconnected,
}
impl Default for WsState {
    fn default() -> Self {
        Self::Disconnected
    }
}

enum UserWsState {
    Connected(user_data::Connection),
    Disconnected,
}
impl Default for UserWsState {
    fn default() -> Self {
        Self::Disconnected
    }
}

#[derive(Debug, Clone, Copy)]
#[derive(Eq, Hash, PartialEq)]
pub enum PaneId {
    HeatmapChart,
    Candlesticks,
    TradePanel,
}

#[derive(Debug, Clone, Copy)]
struct Pane {
    id: PaneId,
    pub is_pinned: bool,
}

impl Pane {
    fn new(id: PaneId) -> Self {
        Self { id, is_pinned: false }
    }
}


fn main() {
    State::run(Settings {
        antialiasing: true,
        ..Settings::default()
    })
    .unwrap();
}

#[derive(Debug, Clone)]
pub enum Message {
    Debug(String),

    // Market&User data stream
    UserListenKey(String),
    UserWsEvent(user_data::Event),
    TickerSelected(Ticker),
    TimeframeSelected(&'static str),
    WsEvent(market_data::Event),
    WsToggle(),
    FetchEvent(Result<Vec<market_data::Kline>, std::string::String>),
    
    // Pane grid
    Split(pane_grid::Axis, PaneId),
    Clicked(pane_grid::Pane),
    Dragged(pane_grid::DragEvent),
    Resized(pane_grid::ResizeEvent),
    TogglePin(pane_grid::Pane),
    Maximize(pane_grid::Pane),
    Restore,
    Close(pane_grid::Pane),
    CloseFocused,
    ToggleLayoutLock,

    // Trading order form
    LimitOrder(String),
    MarketOrder(String),
    CancelOrder(String),
    InputChanged((String, String)),
    OrderCreated(user_data::NewOrder),
    MarketOrderCreated(user_data::NewOrder),
    OrdersFetched(Vec<user_data::NewOrder>),
    OrderFailed(String),

    // Trading table
    TabSelected(usize, String),
    SyncHeader(scrollable::AbsoluteOffset),
    TableResizing(usize, f32),
    TableResized,
    FooterEnabled(bool),
    MinWidthEnabled(bool),

    // Font
    FontLoaded(Result<(), font::Error>),
}

struct State {
    trades_chart: Option<heatmap::LineChart>,
    candlestick_chart: Option<candlesticks::CandlestickChart>,

    // data streams
    listen_key: String,
    selected_ticker: Option<Ticker>,
    selected_timeframe: Option<&'static str>,
    ws_state: WsState,
    user_ws_state: UserWsState,
    ws_running: bool,

    // pane grid
    panes_open: HashMap<PaneId, bool>,
    panes: pane_grid::State<Pane>,
    panes_created: usize,
    focus: Option<pane_grid::Pane>,
    first_pane: pane_grid::Pane,
    pane_lock: bool,

    // order form
    qty_input_val: RefCell<Option<String>>,
    price_input_val: RefCell<Option<String>>,

    // table
    order_form_active_tab: usize,
    table_active_tab: usize,
    open_orders: Vec<user_data::NewOrder>,
    orders_header: scrollable::Id,
    orders_body: scrollable::Id,
    orders_footer: scrollable::Id,
    orders_columns: Vec<TableColumn>,
    orders_rows: Vec<TableRow>,
    pos_header: scrollable::Id,
    pos_body: scrollable::Id,
    pos_footer: scrollable::Id,
    position_columns: Vec<PosTableColumn>,
    position_rows: Vec<PosTableRow>,
    resize_columns_enabled: bool,
    footer_enabled: bool,
    min_width_enabled: bool,
}

impl Application for State {
    type Message = self::Message;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let (panes, first_pane) = pane_grid::State::new(Pane::new(PaneId::TradePanel));

        let mut panes_open = HashMap::new();
        panes_open.insert(PaneId::HeatmapChart, false);
        panes_open.insert(PaneId::Candlesticks, false);
        panes_open.insert(PaneId::TradePanel, true);
        (
            Self { 
                trades_chart: None,
                candlestick_chart: None,
                listen_key: "".to_string(),
                selected_ticker: None,
                selected_timeframe: Some("1m"),
                ws_state: WsState::Disconnected,
                user_ws_state: UserWsState::Disconnected,
                ws_running: false,
                panes,
                panes_open,
                panes_created: 1,
                focus: None,
                first_pane,
                pane_lock: false,
                qty_input_val: RefCell::new(None),
                price_input_val: RefCell::new(None),
                order_form_active_tab: 0,
                table_active_tab: 0,
                open_orders: vec![],
                orders_header: scrollable::Id::unique(),
                orders_body: scrollable::Id::unique(),
                orders_footer: scrollable::Id::unique(),
                pos_header: scrollable::Id::unique(),
                pos_body: scrollable::Id::unique(),
                pos_footer: scrollable::Id::unique(),
                resize_columns_enabled: true,
                footer_enabled: true,
                min_width_enabled: true,
                orders_columns: vec![
                    TableColumn::new(ColumnKind::UpdateTime),
                    TableColumn::new(ColumnKind::Symbol),
                    TableColumn::new(ColumnKind::OrderType),
                    TableColumn::new(ColumnKind::Side),
                    TableColumn::new(ColumnKind::Price),
                    TableColumn::new(ColumnKind::OrigQty),
                    TableColumn::new(ColumnKind::ExecutedQty),
                    TableColumn::new(ColumnKind::ReduceOnly),
                    TableColumn::new(ColumnKind::TimeInForce),
                    TableColumn::new(ColumnKind::CancelOrder),
                ],
                orders_rows: vec![],
                position_columns: vec![
                    PosTableColumn::new(PosColumnKind::Symbol),
                    PosTableColumn::new(PosColumnKind::PosSize),
                    PosTableColumn::new(PosColumnKind::EntryPrice),
                    PosTableColumn::new(PosColumnKind::Breakeven),
                    PosTableColumn::new(PosColumnKind::MarkPrice),
                    PosTableColumn::new(PosColumnKind::LiqPrice),
                    PosTableColumn::new(PosColumnKind::MarginAmt),
                    PosTableColumn::new(PosColumnKind::uPnL),
                ],
                position_rows: vec![],
            },
            Command::batch(vec![
                font::load(ICON_BYTES).map(Message::FontLoaded),
                Command::perform(user_data::get_listen_key(API_KEY, SECRET_KEY), |res| {
                    match res {
                        Ok(listen_key) => {
                            Message::UserListenKey(listen_key)
                        },
                        Err(err) => {
                            eprintln!("Error getting listen key: {}", err);
                            Message::UserListenKey("".to_string())
                        }
                    }
                }),
            ]),
        )
    }

    fn title(&self) -> String {
        "Iced Trade".to_owned()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::TabSelected(index, tab_type) => {
                if tab_type == "order_form" {
                    self.order_form_active_tab = index;
                } else if tab_type == "table" {
                    self.table_active_tab = index;
                }
                Command::none()
            },
            Message::TickerSelected(ticker) => {
                self.selected_ticker = Some(ticker);
                Command::none()
            },
            Message::TimeframeSelected(timeframe) => {
                self.selected_timeframe = Some(timeframe);
                Command::none()
            },
            Message::FontLoaded(_) => {
                dbg!("Font loaded");
                Command::none()
            },
            Message::WsToggle() => {
                self.ws_running =! self.ws_running;
                dbg!(&self.ws_running);
                if self.ws_running {
                    self.trades_chart = Some(LineChart::new());
                    let fetch_klines = Command::perform(
                        market_data::fetch_klines(self.selected_ticker.unwrap().to_string(), self.selected_timeframe.unwrap().to_string())
                            .map_err(|err| format!("{}", err)), 
                        |klines| {
                            Message::FetchEvent(klines)
                        }
                    );
                    let fetch_open_orders = Command::perform(
                        user_data::fetch_open_orders(self.selected_ticker.unwrap().to_string(), API_KEY, SECRET_KEY)
                            .map_err(|err| format!("{}", err)),
                        |orders| {
                            match orders {
                                Ok(orders) => {
                                    Message::OrdersFetched(orders)
                                },
                                Err(err) => {
                                    Message::OrderFailed(format!("{}", err))
                                }
                            }
                        }
                    );
                    let fetch_open_positions = Command::perform(
                        user_data::fetch_open_positions(API_KEY, SECRET_KEY)
                            .map_err(|err| format!("{:?}", err)),
                        |positions| {
                            match positions {
                                Ok(positions) => {
                                    Message::UserWsEvent(user_data::Event::FetchedPositions(positions))
                                },
                                Err(err) => {
                                    Message::OrderFailed(format!("{}", err))
                                }
                            }
                        }
                    );
                    if self.panes.len() == 1 {
                        let first_pane = self.first_pane;
                        let split_pane = Command::perform(
                            async move {
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                (pane_grid::Axis::Horizontal, first_pane) 
                            },
                            |(axis, _pane)| {
                                Message::Split(axis, PaneId::HeatmapChart)
                            }
                        );
                        let split_pane_again = Command::perform(
                            async move {
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                (pane_grid::Axis::Vertical, first_pane) 
                            },
                            |(axis, _pane)| {
                                Message::Split(axis, PaneId::Candlesticks)
                            }
                        );
                        self.panes_open.insert(PaneId::HeatmapChart, true);
                        self.panes_open.insert(PaneId::Candlesticks, true);
                
                        Command::batch(vec![fetch_klines, fetch_open_orders, fetch_open_positions, split_pane, split_pane_again])
                    } else {
                        Command::batch(vec![fetch_klines, fetch_open_orders, fetch_open_positions])
                    }
                } else {
                    self.trades_chart = None;
                    self.candlestick_chart = None;
                    self.open_orders.clear();
                    self.orders_rows.clear();
                    self.position_rows.clear();

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
                market_data::Event::Connected(connection) => {
                    self.ws_state = WsState::Connected(connection);
                    Command::none()
                }
                market_data::Event::Disconnected => {
                    self.ws_state = WsState::Disconnected;
                    Command::none()
                }
                market_data::Event::DepthReceived(depth_update, bids, asks, trades_buffer) => {
                    if let Some(chart) = &mut self.trades_chart {
                        chart.update(depth_update, trades_buffer, bids, asks);
                    }
                    Command::none()
                }
                market_data::Event::KlineReceived(kline) => {
                    if let Some(chart) = &mut self.candlestick_chart {
                        chart.update(kline);
                    }
                    Command::none()
                }
            }, 
            Message::UserWsEvent(event) => {
                match event {
                    user_data::Event::Connected(connection) => {
                        self.user_ws_state = UserWsState::Connected(connection);
                    }
                    user_data::Event::Disconnected => {
                        self.user_ws_state = UserWsState::Disconnected;
                    }
                    user_data::Event::CancelOrder(order_trade_update) => {
                        TableRow::remove_row(order_trade_update.order_id, &mut self.orders_rows);
                    }
                    user_data::Event::NewOrder(order) => {
                        dbg!(order);
                    }
                    user_data::Event::TestEvent(msg) => {
                        dbg!(msg);
                    }
                    user_data::Event::NewPositions(positions) => {
                        for position in positions {
                            PosTableRow::remove_row(position.symbol.clone(), &mut self.position_rows);
                            if position.pos_amt != 0.0 {
                                self.position_rows.push(PosTableRow::add_row(position));
                            }
                        }
                    },
                    user_data::Event::FetchedPositions(positions) => {
                        self.position_rows.clear();
                    
                        for fetched_position in positions {
                            if fetched_position.pos_amt != 0.0 {
                                let position = user_data::Position {
                                    symbol: fetched_position.symbol.clone(),
                                    pos_amt: fetched_position.pos_amt,
                                    entry_price: fetched_position.entry_price,
                                    breakeven_price: fetched_position.breakeven_price,
                                    cum_realized_pnl: String::from("test"),
                                    unrealized_pnl: fetched_position.unrealized_pnl.to_string(),
                                    margin_type: fetched_position.margin_type.clone(),
                                    isolated_wallet: String::from("test"),
                                    pos_side: String::from("test"),
                                };
                    
                                self.position_rows.push(PosTableRow::add_row(position));
                            }
                        }
                    },
                }
                Command::none()
            },
            Message::UserListenKey(listen_key) => {
                if listen_key != "" {
                    self.listen_key = listen_key;
                } else {
                    eprintln!("Error getting listen key");
                }
                Command::none()
            },
            Message::Split(axis, pane_id) => {
                let existing_pane = self.focus.unwrap_or(self.first_pane);

                let result =
                    self.panes.split(axis, existing_pane, Pane::new(pane_id));

                if let Some((pane, _)) = result {
                    self.focus = Some(pane);
                }

                self.panes_open.insert(pane_id, true);

                self.panes_created += 1;
                Command::none()
            },
            Message::Clicked(pane) => {
                self.focus = Some(pane);
                Command::none()
            },
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Command::none()
            },
            Message::Dragged(pane_grid::DragEvent::Dropped {
                pane,
                target,
            }) => {
                self.panes.drop(pane, target);
                Command::none()
            },
            Message::Dragged(_) => {
                Command::none()
            },
            Message::TogglePin(pane) => {
                if let Some(Pane { is_pinned, .. }) = self.panes.get_mut(pane) {
                    *is_pinned = !*is_pinned;
                }
                Command::none()
            },
            Message::Maximize(pane) => {
                self.panes.maximize(pane);
                Command::none()
            },
            Message::Restore => {
                self.panes.restore();
                Command::none()
            },
            Message::Close(pane) => {
                self.panes.get(pane).map(|pane| {
                    match pane.id {
                        PaneId::HeatmapChart => {
                            self.panes_open.insert(PaneId::HeatmapChart, false);
                        },
                        PaneId::Candlesticks => {
                            self.panes_open.insert(PaneId::Candlesticks, false);
                        },
                        PaneId::TradePanel => {
                            self.panes_open.insert(PaneId::TradePanel, false);
                        },
                    }
                });
                
                if let Some((_, sibling)) = self.panes.close(pane) {
                    self.focus = Some(sibling);
                    self.first_pane = sibling;
                }
                Command::none()
            },
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
            },
            Message::ToggleLayoutLock => {
                self.focus = None;
                self.pane_lock = !self.pane_lock;
                self.resize_columns_enabled = !self.pane_lock;
                Command::none()
            },
            Message::LimitOrder(side) => {
                Command::perform(
                    user_data::create_limit_order(side, self.qty_input_val.borrow().as_ref().unwrap().to_string(), self.price_input_val.borrow().as_ref().unwrap().to_string(), API_KEY, SECRET_KEY),
                    |res| {
                        match res {
                            Ok(res) => {
                                Message::OrderCreated(res)
                            },
                            Err(user_data::BinanceError::Reqwest(err)) => {
                                Message::OrderFailed(format!("Network error: {}", err))
                            },
                            Err(user_data::BinanceError::BinanceAPI(err_msg)) => {
                                Message::OrderFailed(format!("Binance API error: {}", err_msg))
                            }
                        }
                    }
                )
            },
            Message::MarketOrder(side) => {
                Command::perform(
                    user_data::create_market_order(side, self.qty_input_val.borrow().as_ref().unwrap().to_string(), API_KEY, SECRET_KEY),
                    |res| {
                        match res {
                            Ok(res) => {
                                Message::MarketOrderCreated(res)
                            },
                            Err(user_data::BinanceError::Reqwest(err)) => {
                                Message::OrderFailed(format!("Network error: {}", err))
                            },
                            Err(user_data::BinanceError::BinanceAPI(err_msg)) => {
                                Message::OrderFailed(format!("Binance API error: {}", err_msg))
                            }
                        }
                    }
                )
            },
            Message::CancelOrder(order_id) => {
                Command::perform(
                    user_data::cancel_order(order_id, API_KEY, SECRET_KEY),
                    |res| {
                        match res {
                            Ok(_) => {
                                Message::OrderFailed("Order cancelled".to_string())
                            },
                            Err(user_data::BinanceError::Reqwest(err)) => {
                                Message::OrderFailed(format!("Network error: {}", err))
                            },
                            Err(user_data::BinanceError::BinanceAPI(err_msg)) => {
                                Message::OrderFailed(format!("Binance API error: {}", err_msg))
                            }
                        }
                    }
                )
            },
            Message::OrdersFetched(orders) => {
                for order in orders {
                    self.open_orders.push(order.clone());
                    self.orders_rows.push(TableRow::add_row(order));
                }
                Command::none()
            },
            Message::OrderCreated(order) => {
                self.orders_rows.push(TableRow::add_row(order.clone()));
                self.open_orders.push(order);
                Command::none()
            },
            Message::MarketOrderCreated(order) => {
                dbg!(order);
                Command::none()
            },
            Message::OrderFailed(err) => {
                eprintln!("Error creating order: {}", err);
                Command::none()
            },
            Message::InputChanged((field, new_value)) => {
                if field == "price" {
                    *self.price_input_val.borrow_mut() = Some(new_value);
                } else if field == "qty" {
                    *self.qty_input_val.borrow_mut() = Some(new_value);
                }
                Command::none()
            },
            Message::SyncHeader(offset) => {
                let orders_batch = Command::batch(vec![
                    scrollable::scroll_to(self.orders_header.clone(), offset),
                    scrollable::scroll_to(self.orders_footer.clone(), offset),
                ]);
                let positions_batch = Command::batch(vec![
                    scrollable::scroll_to(self.pos_header.clone(), offset),
                    scrollable::scroll_to(self.pos_footer.clone(), offset),
                ]);

                if self.table_active_tab == 0 {
                    orders_batch
                } else if self.table_active_tab == 1 {
                    positions_batch
                } else {
                    Command::none()
                }
            },
            Message::TableResizing(index, offset) => {
                if self.table_active_tab == 0 {
                    self.orders_columns[index].resize_offset = Some(offset);
                } else if self.table_active_tab == 1 {
                    self.position_columns[index].resize_offset = Some(offset);
                }
                Command::none()
            },
            Message::TableResized => {
                if self.table_active_tab == 0 {
                    self.orders_columns.iter_mut().for_each(|column| {
                        if let Some(offset) = column.resize_offset.take() {
                            column.width += offset;
                        }
                    });
                } else if self.table_active_tab == 1 {
                    self.position_columns.iter_mut().for_each(|column| {
                        if let Some(offset) = column.resize_offset.take() {
                            column.width += offset;
                        }
                    });
                }
                Command::none()
            },
            Message::FooterEnabled(enabled) => {
                self.footer_enabled = enabled;
                Command::none()
            },
            Message::MinWidthEnabled(enabled) => {
                self.min_width_enabled = enabled;
                Command::none()
            },
            Message::Debug(msg) => {
                dbg!(msg);
                Command::none()
            },
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let focus = self.focus;
        let total_panes = self.panes.len();

        let pane_grid = PaneGrid::new(&self.panes, |id, pane, is_maximized| {
            let is_focused = focus == Some(id);
    
            let content: pane_grid::Content<'_, Message, _, Renderer> = pane_grid::Content::new(responsive(move |size| {
                let pane_id = match pane.id {
                    PaneId::HeatmapChart => PaneId::HeatmapChart,
                    PaneId::Candlesticks => PaneId::Candlesticks,
                    PaneId::TradePanel => PaneId::TradePanel,
                };
                view_content(
                    id, 
                    total_panes, 
                    pane.is_pinned, 
                    size, 
                    pane_id, 
                    &self.trades_chart, 
                    &self.candlestick_chart, 
                    self.qty_input_val.borrow().clone(), 
                    self.price_input_val.borrow().clone(),
                    &self.orders_header,
                    &self.orders_body,
                    &self.pos_header,
                    &self.pos_body,
                    &self.orders_columns,
                    &self.orders_rows,
                    &self.position_columns,
                    &self.position_rows,
                    &self.min_width_enabled,
                    &self.resize_columns_enabled,
                    &self.order_form_active_tab,
                    &self.table_active_tab,
                )
            }));
    
            if self.pane_lock {
                return content.style(style::pane_active);
            }
    
            let mut content = content.style(if is_focused {
                style::pane_focused
            } else {
                style::pane_active
            });
    
            let title = match pane.id {
                PaneId::HeatmapChart => "Heatmap Chart",
                PaneId::Candlesticks => "Candlestick Chart",
                PaneId::TradePanel => "Trading Panel",
            };            
    
            if is_focused {
                let title_bar = pane_grid::TitleBar::new(title)
                    .controls(view_controls(
                        id,
                        total_panes,
                        pane.is_pinned,
                        is_maximized,
                    ))
                    .padding(4)
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

        let ws_button = button(if self.ws_running { "Disconnect" } else { "Connect" })
            .on_press(Message::WsToggle());

        let locked_alt_text = text(char::from(Icon::Locked).to_string()).font(ICON);
        let unlocked_alt_text = text(char::from(Icon::Unlocked).to_string()).font(ICON);
        let layout_lock_button = button(if self.pane_lock { locked_alt_text } else { unlocked_alt_text })
            .on_press(Message::ToggleLayoutLock);

        //let add_new_pane_button = button("+")
        //    .on_press(Message::Split(pane_grid::Axis::Horizontal, self.first_pane));

        let menu_tpl_1 = |items| Menu::new(items).max_width(180.0).offset(15.0).spacing(5.0);

        let mb = menu_bar!(
            (debug_button_s("New Pane"), {
                menu_tpl_1(menu_items!(
                    (debug_button(PaneId::HeatmapChart, self.panes_open.get(&PaneId::HeatmapChart).unwrap_or(&false)))
                    (debug_button(PaneId::Candlesticks, self.panes_open.get(&PaneId::Candlesticks).unwrap_or(&false)))
                    (debug_button(PaneId::TradePanel, self.panes_open.get(&PaneId::TradePanel).unwrap_or(&false)))
                )).width(200.0)
            })
        );

        let mut ws_controls = Row::new()
            .spacing(10)
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
            .spacing(10)
            .align_items(Alignment::Start)
            .width(Length::Fill)
            .height(Length::Fill)
            .push(
                Row::new()
                    .spacing(10)
                    .push(ws_controls)
                    .push(Space::with_width(Length::Fill))
                    .push(mb)                
                    .push(layout_lock_button)
            )
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
        let mut subscriptions = Vec::new();
    
        if let Some(selected_ticker) = &self.selected_ticker {
            if self.ws_running {
                let binance_market_stream = market_data::connect_market_stream(selected_ticker.to_string(), self.selected_timeframe.unwrap().to_string()).map(Message::WsEvent);
                subscriptions.push(binance_market_stream);
            }
        }
        if self.listen_key != "" {
            let binance_user_stream = user_data::connect_user_stream(self.listen_key.clone()).map(Message::UserWsEvent);
            subscriptions.push(binance_user_stream);
        }
        Subscription::batch(subscriptions)
    }    

    fn theme(&self) -> Self::Theme {
        Theme::Oxocarbon
    }
}

fn debug_button<'a>(label: PaneId, is_open: &bool) -> button::Button<'a, Message, iced::Theme, iced::Renderer> {
    if *is_open {
        disabled_labeled_button(&format!("{:?}", label))
    } else {
        labeled_button(&format!("{:?}", label), Message::Split(pane_grid::Axis::Horizontal, label))
    }
}
fn debug_button_s<'a>(label: &str) -> button::Button<'a, Message, iced::Theme, iced::Renderer> {
    labeled_button(label, Message::Debug(label.into())).width(Length::Shrink)
}
fn labeled_button<'a>(
    label: &str,
    msg: Message,
) -> button::Button<'a, Message, iced::Theme, iced::Renderer> {
    base_button(
        text(label).vertical_alignment(alignment::Vertical::Center),
        msg,
    )
}
fn disabled_labeled_button<'a>(
    label: &str,
) -> button::Button<'a, Message, iced::Theme, iced::Renderer> {
    let content = text(label)
        .vertical_alignment(alignment::Vertical::Center);
    button(content)
        .padding([4, 8])
}
fn base_button<'a>(
    content: impl Into<Element<'a, Message, iced::Theme, iced::Renderer>>,
    msg: Message,
) -> button::Button<'a, Message, iced::Theme, iced::Renderer> {
    button(content)
        .padding([4, 8])
        .on_press(msg)
}

fn view_content<'a, 'b: 'a>(
    _pane: pane_grid::Pane,
    _total_panes: usize,
    _is_pinned: bool,
    _size: Size,
    pane_id: PaneId,
    trades_chart: &'a Option<LineChart>,
    candlestick_chart: &'a Option<CandlestickChart>,
    qty_input_val: Option<String>,
    price_input_val: Option<String>, 
    orders_header: &'b scrollable::Id,
    orders_body: &'b scrollable::Id,
    pos_header: &'b scrollable::Id,
    pos_body: &'b scrollable::Id,
    orders_columns: &'b Vec<TableColumn>,
    orders_rows: &'b Vec<TableRow>,
    position_columns: &'b Vec<PosTableColumn>,
    position_rows: &'b Vec<PosTableRow>,
    min_width_enabled: &'b bool,
    resize_columns_enabled: &'b bool,
    order_form_active_tab: &'b usize,
    table_active_tab: &'b usize,
) -> Element<'a, Message> {
    let content = match pane_id {
        PaneId::HeatmapChart => trades_chart.as_ref().map(LineChart::view).unwrap_or_else(|| Text::new("No data").into()),
        PaneId::Candlesticks => candlestick_chart.as_ref().map(CandlestickChart::view).unwrap_or_else(|| Text::new("No data").into()),
        PaneId::TradePanel => {
            let form_select_0_button = button("Market Order")
                .on_press(Message::TabSelected(0, "order_form".to_string()));
            let form_select_1_button = button("Limit Order") 
                .on_press(Message::TabSelected(1, "order_form".to_string()));

            let table_select_0_button = button("Positions")
                .on_press(Message::TabSelected(0, "table".to_string()));
            let table_select_1_button = button("Orders")
                .on_press(Message::TabSelected(1, "table".to_string()));

            let (buy_button, sell_button) = match *order_form_active_tab {
                0 => {
                    (
                        button("Limit Buy").on_press(Message::LimitOrder("BUY".to_string())),
                        button("Limit Sell").on_press(Message::LimitOrder("SELL".to_string()))
                    )
                },
                1 => {
                    (
                        button("Market Buy").on_press(Message::MarketOrder("BUY".to_string())),
                        button("Market Sell").on_press(Message::MarketOrder("SELL".to_string()))
                    )
                },
                _ => {
                    (
                        button("Buy").on_press(Message::LimitOrder("BUY".to_string())),
                        button("Sell").on_press(Message::LimitOrder("SELL".to_string()))
                    )
                },
            };
            let order_buttons = Row::new()
                .push(buy_button)
                .push(sell_button)
                .align_items(Alignment::Center)
                .padding(10)
                .spacing(5);
        
            let qty_input = text_input("Quantity...", qty_input_val.as_deref().unwrap_or(""))
                .on_input(|input| Message::InputChanged(("qty".to_string(), input)));
        
            let inputs = if *order_form_active_tab == 0 {
                let price_input = text_input("Price...", price_input_val.as_deref().unwrap_or(""))
                    .on_input(|input| Message::InputChanged(("price".to_string(), input)));
        
                Row::new()
                    .push(form_select_1_button)
                    .push(price_input)
                    .push(qty_input)                       
                    .push(order_buttons)
                    .align_items(Alignment::Center)
                    .padding(10)
                    .spacing(5)
            } else {
                Row::new()
                    .push(form_select_0_button)
                    .push(qty_input)
                    .push(order_buttons)
                    .align_items(Alignment::Center)
                    .padding(10)
                    .spacing(5)
            };

            if *table_active_tab == 0 {
                let table = responsive(move |size| {
                    let mut table = table(
                        orders_header.clone(),
                        orders_body.clone(),
                        &orders_columns,
                        &orders_rows,
                        Message::SyncHeader,
                    );
                    if *min_width_enabled { table = table.min_width(size.width); }
                    if *resize_columns_enabled {
                        table = table.on_column_resize(Message::TableResizing, Message::TableResized);
                    }
            
                    Container::new(table).padding(10).into()
                });
                Column::new()
                    .push(inputs)
                    .push(table_select_1_button)
                    .push(table)
                    .align_items(Alignment::Center)
                    .into()
            } else {
                let table = responsive(move |size| {
                    let mut table = table(
                        pos_header.clone(),
                        pos_body.clone(),
                        &position_columns,
                        &position_rows,
                        Message::SyncHeader,
                    );
                    if *min_width_enabled { table = table.min_width(size.width); }
                    if *resize_columns_enabled {
                        table = table.on_column_resize(Message::TableResizing, Message::TableResized);
                    }
            
                    Container::new(table).padding(10).into()
                });
                Column::new()
                    .push(inputs)
                    .push(table_select_0_button)
                    .push(table)
                    .align_items(Alignment::Center)
                    .into()
            }        
        },
        _ => Text::new("No data").into(),
    };

    container(content)
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
                (text(char::from(Icon::ResizeSmall).to_string()).font(ICON).size(14), Message::Restore),
                (text(char::from(Icon::Close).to_string()).font(ICON).size(14), Message::Close(pane)),
                //("Split Horizontally", Message::Split(pane_grid::Axis::Horizontal, pane)),
                //("Split Vertically", Message::Split(pane_grid::Axis::Vertical, pane))
            ]
        } else {
            vec![
                (text(char::from(Icon::ResizeFull).to_string()).font(ICON).size(14), Message::Maximize(pane)),
                (text(char::from(Icon::Close).to_string()).font(ICON).size(14), Message::Close(pane)),
                //("Split Horizontally", Message::Split(pane_grid::Axis::Horizontal, pane)),
                //("Split Vertically", Message::Split(pane_grid::Axis::Vertical, pane))
            ]
        };

        for (content, message) in buttons {        
            row = row.push(
                button(content)
                    .padding(3)
                    .on_press(message),
            );
        }
    }
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
struct TableColumn {
    kind: ColumnKind,
    width: f32,
    resize_offset: Option<f32>,
}
impl TableColumn {
    fn new(kind: ColumnKind) -> Self {
        let width = match kind {
            ColumnKind::UpdateTime => 130.0,
            ColumnKind::Symbol => 80.0,
            ColumnKind::OrderType => 50.0,
            ColumnKind::Side => 50.0,
            ColumnKind::Price => 100.0,
            ColumnKind::OrigQty => 80.0,
            ColumnKind::ExecutedQty => 80.0,
            ColumnKind::ReduceOnly => 100.0,
            ColumnKind::TimeInForce => 50.0,
            ColumnKind::CancelOrder => 60.0,
        };

        Self {
            kind,
            width,
            resize_offset: None,
        }
    }
}
enum ColumnKind {
    Symbol,
    Side,
    Price,
    OrigQty,
    ExecutedQty,
    TimeInForce,
    OrderType,
    ReduceOnly,
    UpdateTime,
    CancelOrder
}
struct TableRow {
    order: user_data::NewOrder,
}
impl TableRow {
    fn add_row(order: user_data::NewOrder) -> Self {
        Self {
            order,
        }
    }
    fn update_row(&mut self, order: user_data::NewOrder) {
        self.order = order;
    }
    fn remove_row(order_id: i64, rows: &mut Vec<TableRow>) {
        if let Some(index) = rows.iter().position(|r| r.order.order_id == order_id) {
            rows.remove(index);
        }
    }
}
impl<'a> table::Column<'a, Message, Theme, Renderer> for TableColumn {
    type Row = TableRow;

    fn header(&'a self, _col_index: usize) -> Element<'a, Message> {
        let content = match self.kind {
            ColumnKind::UpdateTime => "Time",
            ColumnKind::Symbol => "Symbol",
            ColumnKind::OrderType => "Type",
            ColumnKind::Side => "Side",
            ColumnKind::Price => "Price",
            ColumnKind::OrigQty => "Amount",
            ColumnKind::ExecutedQty => "Filled",
            ColumnKind::ReduceOnly => "Reduce Only",
            ColumnKind::TimeInForce => "TIF",
            ColumnKind::CancelOrder => "Cancel",
        };

        container(text(content)).height(24).center_y().into()
    }

    fn cell(
        &'a self,
        _col_index: usize,
        row_index: usize,
        row: &'a Self::Row,
    ) -> Element<'a, Message> {
        let content: Element<_> = match self.kind {
            ColumnKind::UpdateTime => text(row.order.update_time.to_string()).into(),
            ColumnKind::Symbol => text(&row.order.symbol).into(),
            ColumnKind::OrderType => text(&row.order.order_type).into(),
            ColumnKind::Side => text(&row.order.side).into(),
            ColumnKind::Price => text(&row.order.price).into(),
            ColumnKind::OrigQty => text(&row.order.orig_qty).into(),
            ColumnKind::ExecutedQty => text(&row.order.executed_qty).into(),
            ColumnKind::ReduceOnly => text(row.order.reduce_only.to_string()).into(),
            ColumnKind::TimeInForce => text(&row.order.time_in_force).into(),
            ColumnKind::CancelOrder => button("X").on_press(Message::CancelOrder(row.order.order_id.to_string())).into(),
        };

        container(content)
            .width(Length::Fill)
            .height(32)
            .center_y()
            .into()
    }

    fn width(&self) -> f32 {
        self.width
    }

    fn resize_offset(&self) -> Option<f32> {
        self.resize_offset
    }
}

struct PosTableColumn {
    kind: PosColumnKind,
    width: f32,
    resize_offset: Option<f32>,
}
impl PosTableColumn {
    fn new(kind: PosColumnKind) -> Self {
        let width = match kind {
            PosColumnKind::Symbol => 100.0,
            PosColumnKind::PosSize => 100.0,
            PosColumnKind::EntryPrice => 100.0,
            PosColumnKind::Breakeven => 100.0,
            PosColumnKind::MarkPrice => 100.0,
            PosColumnKind::LiqPrice => 100.0,
            PosColumnKind::MarginAmt => 100.0,
            PosColumnKind::uPnL => 100.0,
        };

        Self {
            kind,
            width,
            resize_offset: None,
        }
    }
}
enum PosColumnKind {
    Symbol,
    PosSize,
    EntryPrice,
    Breakeven,
    MarkPrice,
    LiqPrice,
    MarginAmt,
    uPnL,
}
#[derive(Debug, Clone)]
struct PosTableRow {
    trade: user_data::Position,
}
impl PosTableRow {
    fn add_row(trade: user_data::Position) -> Self {
        Self {
            trade,
        }
    }
    fn remove_row(symbol: String, rows: &mut Vec<PosTableRow>) {
        if let Some(index) = rows.iter().position(|r| r.trade.symbol == symbol) {
            rows.remove(index);
        }
    }
}
impl<'a> table::Column<'a, Message, Theme, Renderer> for PosTableColumn {
    type Row = PosTableRow;

    fn header(&'a self, _col_index: usize) -> Element<'a, Message> {
        let content = match self.kind {
            PosColumnKind::Symbol => "Symbol",
            PosColumnKind::PosSize => "Size",
            PosColumnKind::EntryPrice => "Entry",
            PosColumnKind::Breakeven => "Breakeven",
            PosColumnKind::MarkPrice => "Mark Price",
            PosColumnKind::LiqPrice => "Liq Price",
            PosColumnKind::MarginAmt => "Margin",
            PosColumnKind::uPnL => "uPnL",
        };

        container(text(content)).height(24).center_y().into()
    }

    fn cell(
        &'a self,
        _col_index: usize,
        row_index: usize,
        row: &'a Self::Row,
    ) -> Element<'a, Message> {
        let content: Element<_> = match self.kind {
            PosColumnKind::Symbol => text(row.trade.symbol.to_string()).into(),
            PosColumnKind::PosSize => text(&row.trade.pos_amt).into(),
            PosColumnKind::EntryPrice => text(&row.trade.entry_price).into(),
            PosColumnKind::Breakeven => text(&row.trade.breakeven_price).into(),
            PosColumnKind::MarkPrice => text("test").into(),
            PosColumnKind::LiqPrice => text("test").into(),
            PosColumnKind::MarginAmt => text("test").into(),
            PosColumnKind::uPnL => text(&row.trade.unrealized_pnl).into(),
        };

        container(content)
            .width(Length::Fill)
            .height(32)
            .center_y()
            .into()
    }

    fn width(&self) -> f32 {
        self.width
    }

    fn resize_offset(&self) -> Option<f32> {
        self.resize_offset
    }
}