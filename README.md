<div align="center">
  <img height="297" width="480" alt="iced-trade" src="https://github.com/user-attachments/assets/79bd0f07-d97c-4186-921f-2e726dcb2c00">
  <img height="297" width="480" alt="iced-trade" src="https://github.com/user-attachments/assets/c862ba41-71f9-411d-bfe4-97f716c36b56">
</div>

### Currently features:
- Customizable and savable grid layouts
- 4 crypto tickers, from Binance and Bybit; BTCUSDT, ETHUSDT, SOLUSDT, LTCUSDT (perpetual swaps)
- 1m, 3m, 5m, 15m and 30m timeframe selections for candlestick and footprint charts. <sup>Tick based "timeframe" selections is planned</sup>
- Tick size multipliers for price grouping on footprint and heatmap charts
- Size filtering for trades showing up in time&sales tables and heatmap charts
- Each grid (pane) can have its own ticker/exchange pair of streams. You can open up however many panes you want. Though, if for example the layout is filled with more than a reasonable amount of heatmap charts, via the most cpu time demanding chart instance between renders, then performance/resource usage might become a slight concern. But there is no limit to it yet to test out the boundries

<div align="center">
  <img height="200" width="300" alt="iced-trade" src="https://github.com/user-attachments/assets/89894672-4ad6-41a2-ab7f-84c5acdb76a9">
  <img height="235" width="200" alt="iced-trade" src="https://github.com/user-attachments/assets/a93ff39f-e80a-4f87-a99b-d4582f4bb818">
</div>

##### There is no server-side yet. User receives market data directly from exchange APIs
- As historical data, currently it can only fetch OHLCV. So, the footprint chart gets populated via candlesticks but not historical trades. Trades gets inserted to the latest candlestick as we receive them from related websocket stream in real-time

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

<a href="https://github.com/iced-rs/iced">
  <img src="https://gist.githubusercontent.com/hecrj/ad7ecd38f6e47ff3688a38c79fd108f0/raw/74384875ecbad02ae2a926425e9bcafd0695bade/color.svg" width="130px">
</a>
