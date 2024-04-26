use iced::futures;  
use iced::subscription::{self, Subscription};
use reqwest::header::{HeaderMap, HeaderValue};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;
use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use async_tungstenite::tungstenite;

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
    LimitOrder(LimitOrder),
    TestEvent(String),
}

#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<String>);

pub fn connect_user_stream(listen_key: String) -> Subscription<Event> {
    struct Connect;

    subscription::channel(
        std::any::TypeId::of::<Connect>(),
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
        
                        match async_tungstenite::tokio::connect_async(
                            websocket_server,
                        )
                        .await
                        {
                            Ok((websocket, _)) => {
                                state = State::Connected(websocket);
                                dbg!("Connected to user stream");
                            }
                            Err(_) => {
                                tokio::time::sleep(
                                    tokio::time::Duration::from_secs(1),
                                )
                                .await;
                                dbg!("Failed to connect to user stream");
                                let _ = output.send(Event::Disconnected).await;
                            }
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
                                                if data["e"] == "ACCOUNT_UPDATE" {
                                                    let event = Event::TestEvent("Account Update".to_string());
                                                    let _ = output.send(event).await;
                                                } else if data["e"] == "ORDER_TRADE_UPDATE" {
                                                    let event = Event::TestEvent("Order Trade Update".to_string());
                                                    let _ = output.send(event).await;
                                                } else {
                                                    let event = Event::TestEvent("Unknown".to_string());
                                                    let _ = output.send(event).await;
                                                }
                                            },
                                            Err(e) => {
                                                dbg!(e, message);
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        dbg!("Disconnected from user stream");
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

pub enum EventType {
    AccountUpdate,
    OrderTradeUpdate,
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
pub struct LimitOrder {
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

pub async fn create_limit_order (side: String, qty: String, price: String, api_key: &str, secret_key: &str) -> Result<LimitOrder, BinanceError> {
    let params = format!("symbol=BTCUSDT&side={}&type=LIMIT&timeInForce=GTC&quantity={}&price={}&timestamp={}", side, qty, price, Utc::now().timestamp_millis());
    let signature = sign_params(&params, secret_key);

    let url = format!("https://testnet.binancefuture.com/fapi/v1/order?{}&signature={}", params, signature);

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(api_key).unwrap());

    let client = reqwest::Client::new();
    let res = client.post(&url).headers(headers).send().await?;

    if res.status().is_success() {
        let limit_order: LimitOrder = res.json().await.map_err(BinanceError::Reqwest)?;
        Ok(limit_order)
    } else {
        let error_msg: String = res.text().await.map_err(BinanceError::Reqwest)?;
        Err(BinanceError::BinanceAPI(error_msg))
    }
}

pub async fn fetch_open_orders(symbol: String, api_key: &str, secret_key: &str) -> Result<Vec<LimitOrder>, reqwest::Error> {
    let params = format!("timestamp={}&symbol={}", Utc::now().timestamp_millis(), symbol);
    let signature = sign_params(&params, secret_key);

    let url = format!("https://testnet.binancefuture.com/fapi/v1/openOrders?{}&signature={}", params, signature);

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(api_key).unwrap());

    let client = reqwest::Client::new();
    let res = client.get(&url).headers(headers).send().await?;

    let open_orders: Vec<LimitOrder> = res.json().await?;
    Ok(open_orders)
}

pub async fn get_listen_key(api_key: &str, secret_key: &str) -> Result<String, reqwest::Error> {
    let params = format!("timestamp={}", Utc::now().timestamp_millis());
    let signature = sign_params(&params, secret_key);

    let url = format!("https://testnet.binancefuture.com/fapi/v1/listenKey?{}&signature={}", params, signature);

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(api_key).unwrap());

    let client = reqwest::Client::new();
    let res = client.post(&url).headers(headers).send().await?;

    let listen_key: serde_json::Value = res.json().await?;
    Ok(listen_key["listenKey"].as_str().unwrap().to_string())
}

fn sign_params(params: &str, secret_key: &str) -> String {
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(params.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}