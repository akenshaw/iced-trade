<div align="center">
  <img height="400" alt="iced-trade" src="https://github.com/user-attachments/assets/9300b283-03fd-498a-a076-7f540f1f96ab">
</div>

### Currently features:
- Customizable and savable grid layouts
- 4 crypto tickers, from Binance and Bybit; BTCUSDT, ETHUSDT, SOLUSDT, LTCUSDT (perpetual swaps)
- 1m, 3m, 5m, 15m and 30m timeframe selections for candlestick and footprint charts. <sup>Tick based "timeframe" selections is planned</sup>
- Tick size multipliers for price grouping on footprint and heatmap charts
- Size filtering for trades in time&sales tables and heatmap charts
- Each grid(pane) can have its own ticker/exchange pair of stream. You can open up however many panes you want. There is no limit to it yet to test out the boundries, but performance/resource usage might be concern, when layout is filled out with many heatmap charts

<div align="center">
  <img height="200" width="300" alt="iced-trade" src="https://github.com/user-attachments/assets/89894672-4ad6-41a2-ab7f-84c5acdb76a9">
  <img height="235" width="200" alt="iced-trade" src="https://github.com/user-attachments/assets/a93ff39f-e80a-4f87-a99b-d4582f4bb818">
</div>

##### There is no server-side yet. User receives market data directly from exchange APIs
- As historical data, currently it can only fetch OHLCV data. So, for example, the footprint chart gets populated via candlesticks, but not trades. Trades gets inserted to these populated data points, as we receive them from related websocket stream in real-time

## Build from source
The releases might not be up-to-date with newest features.<sup>or bugs :)</sup>
- For that, you could
clone the repository into a directory of your choice and build with cargo.

Requirements:
- [Rust toolchain](https://www.rust-lang.org/tools/install)
- [Git version control system](https://git-scm.com/)

```bash
# Clone the repository
git clone https://github.com/akenshaw/iced-trade

cd iced-trade

# Build and run
cargo build --release
cargo run --release
```
***
> 
<p align="center">
  <img src="https://github.com/akenshaw/iced-trade/assets/63060680/e7b55751-b547-4548-ac95-5348c6c60385" width="404,5" />
</p>

> Starting this project, my intention was to create a lightweight/minimal GUI just for trading, while trying to learn Rust on the way. This idea evolved to "trading on the chart", but while I was experimenting with it and seeing the possibilities, it turned out to be a tool to track market activity. Currently still trying to stabilize charting features, and hoping that fully utilized trading would be easier to implement later on
>>  Also starting out, there were some heavy inspirations from Cryptowatch, as it was my "daily driver" for charting, which was discontinued. Funnily enough, I had no idea back then, but recently heard that Cryptowatch was also made with the same GUI library this app made of, Iced. It was quite a surprise for me, so thanks to Iced, for making all this possible!
<a href="https://github.com/iced-rs/iced">
  <img src="https://gist.githubusercontent.com/hecrj/ad7ecd38f6e47ff3688a38c79fd108f0/raw/74384875ecbad02ae2a926425e9bcafd0695bade/color.svg" width="130px">
</a>
