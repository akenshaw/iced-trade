use iced::{futures, stream};
use futures::stream::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;
use futures::channel::mpsc;
use futures::sink::SinkExt;
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use futures::FutureExt;
use async_tungstenite::tungstenite;

mod string_to_f32 {
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<f32>().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum State {
    Disconnected,
    Connected(
        async_tungstenite::WebSocketStream<
            async_tungstenite::tokio::ConnectStream,
        >,
    ),
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(Connection),
    Disconnected,
    NewOrder(NewOrder),
    CancelOrder(OrderTradeUpdate),
    TestEvent(String),
    NewPositions(Vec<Position>),
    FetchedPositions(Vec<FetchedPosition>),
    FetchedBalance(Vec<FetchedBalance>),
}

#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<String>);

pub fn connect_user_stream(listen_key: String) -> impl Stream<Item = Event> {
    stream::channel(
        100,
        |mut output| async move {
            let mut state = State::Disconnected;     
 
            loop {
                match &mut state {
                    State::Disconnected => {
                        let websocket_server = format!(
                            "wss://stream.binancefuture.com/ws/{}",
                            listen_key
                        );
        
                        if let Ok((websocket, _)) = async_tungstenite::tokio::connect_async(
                            websocket_server,
                        )
                        .await {
                            state = State::Connected(websocket);
                            log::info!("Connected to user stream");
                        } else {
                            tokio::time::sleep(tokio::time::Duration::from_secs(1))
                            .await;
                            log::info!("Failed to connect to user stream");
                            let _ = output.send(Event::Disconnected).await;
                        }
                    }
                    State::Connected(websocket) => {
                        let mut fused_websocket = websocket.by_ref().fuse();

                        futures::select! {
                            received = fused_websocket.select_next_some() => {
                                match received {
                                    Ok(tungstenite::Message::Text(message)) => {
                                        let parsed_message: Result<serde_json::Value, _> = serde_json::from_str(&message);
                                        match parsed_message {
                                            Ok(data) => {
                                                let event;
                                                if data["e"] == "ACCOUNT_UPDATE" {
                                                    if let Some(account_update) = data["a"].as_object() {
                                                        let account_update: AccountUpdate = serde_json::from_value(json!(account_update)).unwrap();
                                                        if account_update.event_type == "ORDER" {
                                                            event = Event::NewPositions(account_update.positions);
                                                        } else {
                                                            event = Event::TestEvent("Account Update".to_string());
                                                        }
                                                    } else {
                                                        event = Event::TestEvent("Unknown".to_string());
                                                    }
                                                } else if data["e"] == "ORDER_TRADE_UPDATE" {
                                                    if let Some(order_trade_update) = data["o"].as_object() {
                                                        let order_trade_update: OrderTradeUpdate = serde_json::from_value(json!(order_trade_update)).unwrap();
                                                        if order_trade_update.exec_type == "NEW" {
                                                            event = Event::TestEvent("New Order".to_string());
                                                        } else if order_trade_update.exec_type == "TRADE" {
                                                            event = Event::TestEvent("Trade".to_string());
                                                        } else if order_trade_update.exec_type == "CANCELED" {
                                                            event = Event::CancelOrder(order_trade_update);
                                                        } else {
                                                            event = Event::TestEvent("Unknown".to_string());
                                                        }
                                                    } else {
                                                        event = Event::TestEvent("Unknown".to_string());
                                                    }

                                                } else {
                                                    event = Event::TestEvent("Unknown".to_string());
                                                }
                                                let _ = output.send(event).await;
                                            },
                                            Err(e) => {
                                                log::error!("Failed to parse message: {e:?}");
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        log::info!("Disconnected from user stream");
                                        let _ = output.send(Event::Disconnected).await;
                                        state = State::Disconnected;
                                    }
                                    Ok(_) => continue,
                                }
                            }
                        }
                    }
                }
            }
        },
    )
}

pub fn fetch_user_stream(api_key: &str, secret_key: &str) -> impl Stream<Item = Event> {
    let api_key = api_key.to_owned();
    let secret_key = secret_key.to_owned();

    stream::channel(
        100,
        move |mut output| {
            tokio::spawn(async move {
                loop {
                    let fetch_positions = fetch_open_positions(&api_key, &secret_key);
                    let fetch_balance = fetch_acc_balance(&api_key, &secret_key);

                    let (fetched_positions, fetched_balance) = futures::join!(fetch_positions, fetch_balance);

                    match fetched_positions {
                        Ok(positions) => {
                            let _ = output.send(Event::FetchedPositions(positions)).await;
                        }
                        Err(e) => {
                            log::error!("Error fetching positions: {e:?}");
                        }
                    }

                    match fetched_balance {
                        Ok(balance) => {
                            let _ = output.send(Event::FetchedBalance(balance)).await;
                        }
                        Err(e) => {
                            log::error!("Error fetching balance: {e:?}");
                        }
                    }

                    tokio::time::sleep(std::time::Duration::from_secs(19)).await;
                }
            })
        }.map(|result| result.expect("Failed to join"))
    )
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccBalance {
    #[serde(rename = "a")]
    pub asset: String,
    #[serde(rename = "wb")]
    pub wallet_bal: String,
    #[serde(rename = "cw")]
    pub cross_bal: String,
    #[serde(rename = "bc")]
    pub balance_chg: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct FetchedBalance {
    pub asset: String,
    #[serde(with = "string_to_f32", rename = "balance")]
    pub balance: f32,
    #[serde(with = "string_to_f32", rename = "crossWalletBalance")]
    pub cross_bal: f32,
    #[serde(with = "string_to_f32", rename = "crossUnPnl")]
    pub cross_upnl: f32,
    #[serde(with = "string_to_f32", rename = "availableBalance")]
    pub available_bal: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(with = "string_to_f32", rename = "pa")]
    pub pos_amt: f32,
    #[serde(with = "string_to_f32", rename = "ep")]
    pub entry_price: f32,
    #[serde(with = "string_to_f32", rename = "bep")]
    pub breakeven_price: f32,
    #[serde(rename = "up")]
    pub unrealized_pnl: String,
    #[serde(rename = "mt")]
    pub margin_type: String,
    #[serde(rename = "iw")]
    pub isolated_wallet: String,
    #[serde(rename = "ps")]
    pub pos_side: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FetchedPosition {
    pub symbol: String,
    #[serde(with = "string_to_f32", rename = "positionAmt")]
    pub pos_amt: f32,
    #[serde(with = "string_to_f32", rename = "entryPrice")]
    pub entry_price: f32,
    #[serde(with = "string_to_f32", rename = "breakEvenPrice")]
    pub breakeven_price: f32,
    #[serde(with = "string_to_f32", rename = "markPrice")]
    pub mark_price: f32,
    #[serde(with = "string_to_f32", rename = "unRealizedProfit")]
    pub unrealized_pnl: f32,
    #[serde(with = "string_to_f32", rename = "liquidationPrice")]
    pub liquidation_price: f32,
    #[serde(with = "string_to_f32", rename = "leverage")]
    pub leverage: f32,
    #[serde(rename = "marginType")]
    pub margin_type: String,
}

#[derive(Debug, Clone)]
pub struct PositionInTable {
    pub symbol: String,
    pub size: f32,
    pub entry_price: f32,
    pub breakeven_price: f32,
    pub mark_price: f32,
    pub liquidation_price: f32,
    pub margin_amt: f32,
    pub unrealized_pnl: f32,
}

pub enum EventType {
    AccountUpdate,
    OrderTradeUpdate,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountUpdate {
    #[serde(rename = "m")]
    pub event_type: String,
    #[serde(rename = "B")]
    pub balances: Vec<AccBalance>,
    #[serde(rename = "P")]
    pub positions: Vec<Position>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderTradeUpdate {
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "S")]
    pub side: String,
    #[serde(rename = "o")]
    pub order_type: String,
    #[serde(rename = "x")]
    pub exec_type: String,
    #[serde(rename = "X")]
    pub order_status: String,
    #[serde(rename = "f")]
    pub time_in_force: String,
    #[serde(rename = "wt")]
    pub working_type: String,
    #[serde(rename = "i")]
    pub order_id: i64,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub orig_qty: String,
}

#[derive(Debug)]
pub enum BinanceError {
    Reqwest(reqwest::Error),
    BinanceAPI(String),
}

impl From<reqwest::Error> for BinanceError {
    fn from(err: reqwest::Error) -> BinanceError {
        BinanceError::Reqwest(err)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewOrder {
    #[serde(rename = "orderId")]
    pub order_id: i64,
    pub symbol: String,
    pub side: String,
    pub price: String,
    #[serde(rename = "origQty")]
    pub orig_qty: String,
    #[serde(rename = "executedQty")]
    pub executed_qty: String,
    #[serde(rename = "timeInForce")]
    pub time_in_force: String,
    #[serde(rename = "type")]
    pub order_type: String,
    #[serde(rename = "reduceOnly")]
    pub reduce_only: bool,
    #[serde(rename = "updateTime")]
    pub update_time: u64,
}

pub async fn create_limit_order (side: String, qty: String, price: String, api_key: &str, secret_key: &str) -> Result<NewOrder, BinanceError> {
    let params = format!("symbol=BTCUSDT&side={}&type=LIMIT&timeInForce=GTC&quantity={}&price={}&timestamp={}", side, qty, price, Utc::now().timestamp_millis());
    let signature = sign_params(&params, secret_key);

    let url = format!("https://testnet.binancefuture.com/fapi/v1/order?{}&signature={}", params, signature);

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(api_key).unwrap());

    let client = reqwest::Client::new();
    let res = client.post(&url).headers(headers).send().await?;

    if res.status().is_success() {
        let limit_order: NewOrder = res.json().await.map_err(BinanceError::Reqwest)?;
        Ok(limit_order)
    } else {
        let error_msg: String = res.text().await.map_err(BinanceError::Reqwest)?;
        Err(BinanceError::BinanceAPI(error_msg))
    }
}

pub async fn create_market_order (side: String, qty: String, api_key: &str, secret_key: &str) -> Result<NewOrder, BinanceError> {
    let params = format!("symbol=BTCUSDT&side={}&type=MARKET&quantity={}&timestamp={}", side, qty, Utc::now().timestamp_millis());
    let signature = sign_params(&params, secret_key);

    let url = format!("https://testnet.binancefuture.com/fapi/v1/order?{params}&signature={signature}");

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(api_key).unwrap());

    let client = reqwest::Client::new();
    let res = client.post(&url).headers(headers).send().await?;

    if res.status().is_success() {
        let market_order: NewOrder = res.json().await.map_err(BinanceError::Reqwest)?;
        Ok(market_order)
    } else {
        let error_msg: String = res.text().await.map_err(BinanceError::Reqwest)?;
        Err(BinanceError::BinanceAPI(error_msg))
    }
}

pub async fn cancel_order(order_id: String, api_key: &str, secret_key: &str) -> Result<(), BinanceError> {
    let params = format!("symbol=BTCUSDT&orderId={}&timestamp={}", order_id, Utc::now().timestamp_millis());
    let signature = sign_params(&params, secret_key);

    let url = format!("https://testnet.binancefuture.com/fapi/v1/order?{params}&signature={signature}");

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(api_key).unwrap());

    let client = reqwest::Client::new();
    let res = client.delete(&url).headers(headers).send().await?;

    if res.status().is_success() {
        Ok(())
    } else {
        let error_msg: String = res.text().await.map_err(BinanceError::Reqwest)?;
        Err(BinanceError::BinanceAPI(error_msg))
    }
}

pub async fn fetch_open_orders(symbol: String, api_key: &str, secret_key: &str) -> Result<Vec<NewOrder>, BinanceError> {
    let params = format!("timestamp={}&symbol={}", Utc::now().timestamp_millis(), symbol);
    let signature = sign_params(&params, secret_key);

    let url = format!("https://testnet.binancefuture.com/fapi/v1/openOrders?{params}&signature={signature}");

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(api_key).unwrap());

    let client = reqwest::Client::new();
    let res = client.get(&url).headers(headers).send().await?;

    let open_orders: Vec<NewOrder> = res.json().await?;
    Ok(open_orders)
}

pub async fn fetch_open_positions(api_key: &str, secret_key: &str) -> Result<Vec<FetchedPosition>, BinanceError> {
    let params = format!("timestamp={}", Utc::now().timestamp_millis());
    let signature = sign_params(&params, secret_key);

    let url = format!("https://testnet.binancefuture.com/fapi/v2/positionRisk?{params}&signature={signature}");

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(api_key).unwrap());

    let client = reqwest::Client::new();
    let res = client.get(&url).headers(headers).send().await?;

    let positions: Vec<FetchedPosition> = res.json().await?;

    Ok(positions)
}

pub async fn fetch_acc_balance(api_key: &str, secret_key: &str) -> Result<Vec<FetchedBalance>, BinanceError> {
    let params = format!("timestamp={}", Utc::now().timestamp_millis());
    let signature = sign_params(&params, secret_key);

    let url = format!("https://testnet.binancefuture.com/fapi/v2/balance?{params}&signature={signature}");

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(api_key).unwrap());

    let client = reqwest::Client::new();
    let res = client.get(&url).headers(headers).send().await?;

    let acc_balance: Vec<FetchedBalance> = res.json().await?;
    Ok(acc_balance)
}

pub async fn get_listen_key(api_key: &str, secret_key: &str) -> Result<String, BinanceError> {
    let params = format!("timestamp={}", Utc::now().timestamp_millis());
    let signature = sign_params(&params, secret_key);

    let url = format!("https://testnet.binancefuture.com/fapi/v1/listenKey?{params}&signature={signature}");

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(api_key).unwrap());

    let client = reqwest::Client::new();
    let res = client.post(&url).headers(headers).send().await?;

    let listen_key: serde_json::Value = res.json().await?;
    
    if let Some(key) = listen_key.get("listenKey") {
        Ok(key.as_str().unwrap().to_string())
    } else {
        Err(BinanceError::BinanceAPI("Failed to get listen key".to_string()))
    }
}

fn sign_params(params: &str, secret_key: &str) -> String {
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(params.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}