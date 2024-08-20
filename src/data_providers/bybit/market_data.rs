use iced::{stream, futures};
use futures::sink::SinkExt;
use futures::stream::Stream;

use serde_json::Value;
use bytes::Bytes;

use sonic_rs::{JsonValueTrait, Deserialize, Serialize}; 
use sonic_rs::to_object_iter_unchecked;

use anyhow::anyhow;
use anyhow::{Context, Result};

use fastwebsockets::{Frame, FragmentCollector, OpCode};
use http_body_util::Empty;
use hyper::header::{CONNECTION, UPGRADE};
use hyper::upgrade::Upgraded;
use hyper::Request;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio_rustls::rustls::{ClientConfig, OwnedTrustAnchor};
use tokio_rustls::TlsConnector;

use crate::data_providers::{Depth, FeedLatency, Kline, LocalDepthCache, Order, Trade};
use crate::{Ticker, Timeframe};

#[allow(clippy::large_enum_variant)]
enum State {
    Disconnected,
    Connected(
        FragmentCollector<TokioIo<Upgraded>>
    ),
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(Connection),
    Disconnected(String),
    DepthReceived(Ticker, FeedLatency, i64, Depth, Vec<Trade>),
    KlineReceived(Ticker, Kline, Timeframe),
}

#[derive(Debug, Clone)]
pub struct Connection;

#[derive(Serialize, Deserialize, Debug)]
struct SonicDepth {
	#[serde(rename = "u")]
	pub update_id: u64,
	#[serde(rename = "b")]
	pub bids: Vec<BidAsk>,
	#[serde(rename = "a")]
	pub asks: Vec<BidAsk>,
}

#[derive(Serialize, Deserialize, Debug)]
struct BidAsk {
	#[serde(rename = "0")]
	pub price: String,
	#[serde(rename = "1")]
	pub qty: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct SonicTrade {
	#[serde(rename = "T")]
	pub time: u64,
	#[serde(rename = "p")]
	pub price: String,
	#[serde(rename = "v")]
	pub qty: String,
	#[serde(rename = "S")]
	pub is_sell: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SonicKline {
    #[serde(rename = "start")]
    pub time: u64,
    #[serde(rename = "open")]
    pub open: String,
    #[serde(rename = "high")]
    pub high: String,
    #[serde(rename = "low")]
    pub low: String,
    #[serde(rename = "close")]
    pub close: String,
    #[serde(rename = "volume")]
    pub volume: String,
    #[serde(rename = "interval")]
    pub interval: String,
}

#[derive(Debug)]
enum StreamData {
	Trade(Vec<SonicTrade>),
	Depth(SonicDepth, String, i64),
    Kline(Ticker, Vec<SonicKline>),
}

#[derive(Debug)]
enum StreamName {
    Depth(Ticker),
    Trade(Ticker),
    Kline(Ticker),
    Unknown,
}
impl StreamName {
    fn from_topic(topic: &str) -> Self {
        topic.split('.').collect::<Vec<&str>>().as_slice().split_first().map(|(first, rest)| {
            match *first {
                "publicTrade" => {
                    match rest {
                        [ticker] if *ticker == "BTCUSDT" => StreamName::Trade(Ticker::BTCUSDT),
                        [ticker] if *ticker == "ETHUSDT" => StreamName::Trade(Ticker::ETHUSDT),
                        [ticker] if *ticker == "SOLUSDT" => StreamName::Trade(Ticker::SOLUSDT),
                        [ticker] if *ticker == "LTCUSDT" => StreamName::Trade(Ticker::LTCUSDT),
                        _ => StreamName::Unknown,
                    }
                },
                "orderbook" => {
                    match rest {
                        [_, ticker] if *ticker == "BTCUSDT" => StreamName::Depth(Ticker::BTCUSDT),
                        [_, ticker] if *ticker == "ETHUSDT" => StreamName::Depth(Ticker::ETHUSDT),
                        [_, ticker] if *ticker == "SOLUSDT" => StreamName::Depth(Ticker::SOLUSDT),
                        [_, ticker] if *ticker == "LTCUSDT" => StreamName::Depth(Ticker::LTCUSDT),
                        _ => StreamName::Unknown,
                    }
                },
                "kline" => {
                    match rest {
                        [_, ticker] if *ticker == "BTCUSDT" => StreamName::Kline(Ticker::BTCUSDT),
                        [_, ticker] if *ticker == "ETHUSDT" => StreamName::Kline(Ticker::ETHUSDT),
                        [_, ticker] if *ticker == "SOLUSDT" => StreamName::Kline(Ticker::SOLUSDT),
                        [_, ticker] if *ticker == "LTCUSDT" => StreamName::Kline(Ticker::LTCUSDT),
                        _ => StreamName::Unknown,
                    }
                },
                _ => StreamName::Unknown,
            }
        }).unwrap_or(StreamName::Unknown)
    }
}

#[derive(Debug)]
enum StreamWrapper {
	Trade,
	Depth,
    Kline,
}

fn feed_de(bytes: &Bytes) -> Result<StreamData> {
    let mut stream_type: Option<StreamWrapper> = None;

    let mut depth_wrap: Option<SonicDepth> = None;

    let mut data_type: String = String::new();

    let iter: sonic_rs::ObjectJsonIter = unsafe { to_object_iter_unchecked(bytes) };

    let mut topic_ticker = Ticker::BTCUSDT;

    for elem in iter {
        let (k, v) = elem.context("Error parsing stream")?;

        if k == "topic" {
            if let Some(val) = v.as_str() {
                match StreamName::from_topic(val) {
                    StreamName::Depth(ticker) => {
                        stream_type = Some(StreamWrapper::Depth);

                        topic_ticker = ticker;
                    },
                    StreamName::Trade(ticker) => {
                        stream_type = Some(StreamWrapper::Trade);

                        topic_ticker = ticker;
                    },
                    StreamName::Kline(ticker) => {
                        stream_type = Some(StreamWrapper::Kline);

                        topic_ticker = ticker;
                    },
                    _ => {
                        log::error!("Unknown stream name");
                    }
                }
            }
        } else if k == "type" {
            v.as_str().unwrap().clone_into(&mut data_type);
        } else if k == "data" {
            match stream_type {
                Some(StreamWrapper::Trade) => {
                    let trade_wrap: Vec<SonicTrade> = sonic_rs::from_str(&v.as_raw_faststr())
                        .context("Error parsing trade")?;

                    return Ok(StreamData::Trade(trade_wrap));
                },
                Some(StreamWrapper::Depth) => {
                    if depth_wrap.is_none() {
                        depth_wrap = Some(SonicDepth {
                            update_id: 0,
                            bids: Vec::new(),
                            asks: Vec::new(),
                        });
                    }
                    depth_wrap = Some(sonic_rs::from_str(&v.as_raw_faststr())
                        .context("Error parsing depth")?);
                },
                Some(StreamWrapper::Kline) => {
                    let kline_wrap: Vec<SonicKline> = sonic_rs::from_str(&v.as_raw_faststr())
                        .context("Error parsing kline")?;

                    return Ok(StreamData::Kline(topic_ticker, kline_wrap));
                },
                _ => {
                    log::error!("Unknown stream type");
                }
            }
        } else if k == "cts" {
            if let Some(dw) = depth_wrap {
                let time: u64 = v.as_u64().context("Error parsing time")?;
                
                return Ok(StreamData::Depth(dw, data_type.to_string(), time as i64));
            }
        }
    }

    Err(anyhow::anyhow!("Unknown data"))
}

fn tls_connector() -> Result<TlsConnector> {
	let mut root_store = tokio_rustls::rustls::RootCertStore::empty();

	root_store.add_trust_anchors(
		webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
			OwnedTrustAnchor::from_subject_spki_name_constraints(
			ta.subject,
			ta.spki,
			ta.name_constraints,
			)
		}),
	);

	let config = ClientConfig::builder()
		.with_safe_defaults()
		.with_root_certificates(root_store)
		.with_no_client_auth();

	Ok(TlsConnector::from(std::sync::Arc::new(config)))
}

async fn connect(domain: &str) -> Result<FragmentCollector<TokioIo<Upgraded>>> {
	let mut addr = String::from(domain);
    addr.push_str(":443");

	let tcp_stream: TcpStream = TcpStream::connect(&addr).await?;
	let tls_connector: TlsConnector = tls_connector().unwrap();
	let domain: tokio_rustls::rustls::ServerName =
	tokio_rustls::rustls::ServerName::try_from(domain).map_err(|_| {
		std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid dnsname")
	})?;

	let tls_stream: tokio_rustls::client::TlsStream<TcpStream> = tls_connector.connect(domain, tcp_stream).await?;

    let url = "wss://stream.bybit.com/v5/public/linear".to_string();

	let req: Request<Empty<Bytes>> = Request::builder()
	.method("GET")
	.uri(url)
	.header("Host", &addr)
	.header(UPGRADE, "websocket")
	.header(CONNECTION, "upgrade")
	.header(
		"Sec-WebSocket-Key",
		fastwebsockets::handshake::generate_key(),
	)
	.header("Sec-WebSocket-Version", "13")
	.body(Empty::<Bytes>::new())?;

	let (ws, _) = fastwebsockets::handshake::client(&SpawnExecutor, req, tls_stream).await?;
	Ok(FragmentCollector::new(ws))
}
struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
  Fut: std::future::Future + Send + 'static,
  Fut::Output: Send + 'static,
{
  fn execute(&self, fut: Fut) {
	tokio::task::spawn(fut);
  }
}

fn str_f32_parse(s: &str) -> f32 {
    s.parse::<f32>().unwrap_or_else(|e| {
        log::error!("Failed to parse float: {}, error: {}", s, e);
        0.0
    })
}

fn string_to_timeframe(interval: &str) -> Option<Timeframe> {
    Timeframe::ALL.iter().find(|&tf| tf.to_string() == format!("{}m", interval)).copied()
}

pub fn connect_market_stream(ticker: Ticker) -> impl Stream<Item = Event> {
    stream::channel (
        100,
        move |mut output| async move {
            let mut state: State = State::Disconnected;  

            let mut trades_buffer: Vec<Trade> = Vec::new();    

            let selected_ticker = ticker;

            let symbol_str = match selected_ticker {
                Ticker::BTCUSDT => "BTCUSDT",
                Ticker::ETHUSDT => "ETHUSDT",
                Ticker::SOLUSDT => "SOLUSDT",
                Ticker::LTCUSDT => "LTCUSDT",
            };

            let stream_1 = format!("publicTrade.{symbol_str}");
            let stream_2 = format!("orderbook.500.{symbol_str}");

            let mut orderbook: LocalDepthCache = LocalDepthCache::new();

            let mut trade_latencies: Vec<i64> = Vec::new();

            loop {
                match &mut state {
                    State::Disconnected => {        
                        let domain: &str = "stream.bybit.com";

                        if let Ok(mut websocket) = connect(domain
                        )
                        .await {
                            let subscribe_message: String = serde_json::json!({
                                "op": "subscribe",
                                "args": [stream_1, stream_2]
                            }).to_string();
    
                            if let Err(e) = websocket.write_frame(Frame::text(fastwebsockets::Payload::Borrowed(subscribe_message.as_bytes()))).await {
                                let _ = output.send(Event::Disconnected(
                                    format!("Failed subscribing: {}", e)
                                )).await;

                                continue;
                            }

                            state = State::Connected(websocket);
                            let _ = output.send(Event::Connected(Connection)).await; 
                        } else {
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                            let _ = output.send(Event::Disconnected(
                                "Failed to connect to websocket".to_string()
                            )).await;
                        }
                    },
                    State::Connected(websocket) => {
                        let feed_latency: FeedLatency;

                        match websocket.read_frame().await {
                            Ok(msg) => match msg.opcode {
                                OpCode::Text => {       
                                    let json_bytes: Bytes = Bytes::from(msg.payload.to_vec());

                                    if let Ok(data) = feed_de(&json_bytes) {
                                        match data {
                                            StreamData::Trade(de_trade_vec) => {
                                                for de_trade in de_trade_vec.iter() {
                                                    let trade = Trade {
                                                        time: de_trade.time as i64,
                                                        is_sell: de_trade.is_sell == "Sell",
                                                        price: str_f32_parse(&de_trade.price),
                                                        qty: str_f32_parse(&de_trade.qty),
                                                    };

                                                    trade_latencies.push(
                                                        chrono::Utc::now().timestamp_millis() - trade.time
                                                    );

                                                    trades_buffer.push(trade);
                                                }                                             
                                            },
                                            StreamData::Depth(de_depth, data_type, time) => {                                            
                                                let depth_latency = chrono::Utc::now().timestamp_millis() - time;

                                                let depth_update = LocalDepthCache {
                                                    last_update_id: de_depth.update_id as i64,
                                                    time,
                                                    bids: de_depth.bids.iter().map(
                                                        |x| Order { price: str_f32_parse(&x.price), qty: str_f32_parse(&x.qty) }
                                                    ).collect(),
                                                    asks: de_depth.asks.iter().map(
                                                        |x| Order { price: str_f32_parse(&x.price), qty: str_f32_parse(&x.qty) }
                                                    ).collect(),
                                                };

                                                if (data_type == "snapshot") || (depth_update.last_update_id == 1) {
                                                    orderbook.fetched(depth_update);

                                                } else if data_type == "delta" {
                                                    orderbook.update_depth_cache(depth_update);

                                                    let avg_trade_latency = if !trade_latencies.is_empty() {
                                                        let avg = trade_latencies.iter().sum::<i64>() / trade_latencies.len() as i64;
                                                        trade_latencies.clear();
                                                        Some(avg)
                                                    } else {
                                                        None
                                                    };
                                                    feed_latency = FeedLatency {
                                                        time,
                                                        depth_latency,
                                                        trade_latency: avg_trade_latency,
                                                    };

                                                    let _ = output.send(
                                                        Event::DepthReceived(
                                                            selected_ticker,
                                                            feed_latency,
                                                            time, 
                                                            orderbook.get_depth(),
                                                            std::mem::take(&mut trades_buffer)
                                                        )
                                                    ).await;
                                                }
                                            },
                                            _ => {
                                                log::warn!("Unknown data: {:?}", &data);
                                            }
                                        }
                                    }
                                }
                                OpCode::Close => {
                                    state = State::Disconnected;
                                    let _ = output.send(
                                        Event::Disconnected("Connection closed".to_string())
                                    ).await;
                                }
                                _ => {}
                            },
                            Err(e) => {
                                state = State::Disconnected;        
                                let _ = output.send(
                                    Event::Disconnected("Error reading frame: ".to_string() + &e.to_string())
                                ).await;
                            }
                        }
                    }
                }
            }
        },
    )
}
 
pub fn connect_kline_stream(streams: Vec<(Ticker, Timeframe)>) -> impl Stream<Item = Event> {
    stream::channel (
        100,
        move |mut output| async move {
            let mut state = State::Disconnected;    

            let stream_str = streams.iter().map(|(ticker, timeframe)| {
                let symbol_str = match ticker {
                    Ticker::BTCUSDT => "BTCUSDT",
                    Ticker::ETHUSDT => "ETHUSDT",
                    Ticker::SOLUSDT => "SOLUSDT",
                    Ticker::LTCUSDT => "LTCUSDT",
                };
                let timeframe_str = match timeframe {
                    Timeframe::M1 => "1",
                    Timeframe::M3 => "3",
                    Timeframe::M5 => "5",
                    Timeframe::M15 => "15",
                    Timeframe::M30 => "30",
                };
                format!("kline.{timeframe_str}.{symbol_str}")
            }).collect::<Vec<String>>();
 
            loop {
                match &mut state {
                    State::Disconnected => {
                        let domain = "stream.bybit.com";
                        
                        if let Ok(mut websocket) = connect(
                            domain,
                        )
                        .await {
                            let subscribe_message = serde_json::json!({
                                "op": "subscribe",
                                "args": stream_str 
                            }).to_string();
    
                            if let Err(e) = websocket.write_frame(Frame::text(fastwebsockets::Payload::Borrowed(subscribe_message.as_bytes()))).await {
                                let _ = output.send(Event::Disconnected
                                    (format!("Failed subscribing: {}", e))
                                ).await;

                                continue;
                            }

                            state = State::Connected(websocket);
                            let _ = output.send(Event::Connected(Connection)).await;
                        } else {
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                            let _ = output.send(Event::Disconnected(
                                "Failed to connect to websocket".to_string()
                            )).await;
                        }
                    }
                    State::Connected(websocket) => {
                        match websocket.read_frame().await {
                            Ok(msg) => match msg.opcode {
                                OpCode::Text => {                    
                                    let json_bytes: Bytes = Bytes::from(msg.payload.to_vec());
                    
                                    if let Ok(StreamData::Kline(ticker, de_kline_vec)) = feed_de(&json_bytes) {
                                        for de_kline in de_kline_vec.iter() {
                                            let kline = Kline {
                                                time: de_kline.time,
                                                open: str_f32_parse(&de_kline.open),
                                                high: str_f32_parse(&de_kline.high),
                                                low: str_f32_parse(&de_kline.low),
                                                close: str_f32_parse(&de_kline.close),
                                                volume: (-1.0, str_f32_parse(&de_kline.volume)),
                                            };

                                            if let Some(timeframe) = string_to_timeframe(&de_kline.interval) {
                                                let _ = output.send(Event::KlineReceived(ticker, kline, timeframe)).await;
                                            } else {
                                                log::error!("Failed to find timeframe: {}, {:?}", &de_kline.interval, streams);
                                            }
                                        }
                                         
                                    } else {
                                        log::error!("\nUnknown data: {:?}", &json_bytes);
                                    }
                                }
                                _ => {}
                            },
                            Err(e) => {   
                                state = State::Disconnected;             
                                let _ = output.send(
                                    Event::Disconnected("Error reading frame: ".to_string() + &e.to_string())
                                ).await;
                            }
                        }
                    }
                }
            }
        },
    )
}

#[derive(Deserialize, Debug)]
struct ApiResponse {
    #[serde(rename = "retCode")]
    ret_code: u32,
    #[serde(rename = "retMsg")]
    ret_msg: String,
    result: ApiResult,
}

#[derive(Deserialize, Debug)]
struct ApiResult {
    symbol: String,
    category: String,
    list: Vec<Vec<Value>>,
}

pub async fn fetch_klines(ticker: Ticker, timeframe: Timeframe) -> Result<Vec<Kline>> {
    let symbol_str: &str = match ticker {
        Ticker::BTCUSDT => "BTCUSDT",
        Ticker::ETHUSDT => "ETHUSDT",
        Ticker::SOLUSDT => "SOLUSDT",
        Ticker::LTCUSDT => "LTCUSDT",
    };
    let timeframe_str: &str = match timeframe {
        Timeframe::M1 => "1",
        Timeframe::M3 => "3",
        Timeframe::M5 => "5",
        Timeframe::M15 => "15",
        Timeframe::M30 => "30",
    };

    let url: String = format!("https://api.bybit.com/v5/market/kline?category=linear&symbol={symbol_str}&interval={timeframe_str}&limit=720");

    let response: reqwest::Response = reqwest::get(&url).await
        .context("Failed to send request")?;
    let text: String = response.text().await
        .context("Failed to read response text")?;

    let api_response: ApiResponse = sonic_rs::from_str(&text)
        .context("Failed to parse JSON")?;
    
    let klines: Result<Vec<Kline>, anyhow::Error> = api_response.result.list.iter().map(|kline| {
        let time = kline[0].as_str().ok_or_else(|| anyhow!("Missing time value"))
            .and_then(|s| s.parse::<u64>()
            .context("Failed to parse time as u64"));
        let open = kline[1].as_str().ok_or_else(|| anyhow!("Missing open value"))
            .and_then(|s| s.parse::<f32>()
            .context("Failed to parse open as f32"));
        let high = kline[2].as_str().ok_or_else(|| anyhow!("Missing high value"))
            .and_then(|s| s.parse::<f32>()
            .context("Failed to parse high as f32"));
        let low = kline[3].as_str().ok_or_else(|| anyhow!("Missing low value"))
            .and_then(|s| s.parse::<f32>()
            .context("Failed to parse low as f32"));
        let close = kline[4].as_str().ok_or_else(|| anyhow!("Missing close value"))
            .and_then(|s| s.parse::<f32>()
            .context("Failed to parse close as f32"));
        let volume = kline[5].as_str().ok_or_else(|| anyhow!("Missing volume value"))
            .and_then(|s| s.parse::<f32>()
            .context("Failed to parse volume as f32"));
    
        Ok(Kline {
            time: time?,
            open: open?,
            high: high?,
            low: low?,
            close: close?,
            volume: (-1.0, volume?),
        })
    }).collect();

    klines
}

pub async fn fetch_ticksize(ticker: Ticker) -> Result<f32> {
    let symbol_str = match ticker {
        Ticker::BTCUSDT => "BTCUSDT",
        Ticker::ETHUSDT => "ETHUSDT",
        Ticker::SOLUSDT => "SOLUSDT",
        Ticker::LTCUSDT => "LTCUSDT",
    };

    let url = format!("https://api.bybit.com/v5/market/instruments-info?category=linear&symbol={}", symbol_str);

    let response: reqwest::Response = reqwest::get(&url).await
        .context("Failed to send request")?;
    let text: String = response.text().await
        .context("Failed to read response text")?;

    let exchange_info: Value = sonic_rs::from_str(&text)
        .context("Failed to parse JSON")?;

    let result_list: &Vec<Value> = exchange_info["result"]["list"].as_array()
        .context("Result list is not an array")?;

    for item in result_list {
        if item["symbol"] == symbol_str {
            let price_filter: &serde_json::Map<String, Value> = item["priceFilter"].as_object()
                .context("Price filter not found")?;

            let tick_size_str: &str = price_filter.get("tickSize").context("Tick size not found")?.as_str()
                .context("Tick size is not a string")?;

            return tick_size_str.parse::<f32>()
                .context("Failed to parse tick size");
        }
    }

    anyhow::bail!("Tick size not found for symbol {}", symbol_str)
}