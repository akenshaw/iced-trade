<p align="center">
  <img width="485,4" height="300" alt="Screenshot 2024-06-08 at 12 13 28 AM" src="https://github.com/akenshaw/iced-trade/assets/63060680/6b3217fe-1ef4-43c1-94c0-9b11daff372a">  
  <img src="https://github.com/akenshaw/iced-trade/assets/63060680/158e4bb1-0bd3-435e-a1b5-6ccae62d9d5c" width="400" height="300" style="display: inline-block;" />
  <img src="https://github.com/akenshaw/iced-trade/assets/63060680/df84809e-7ddc-42ad-85fc-a90dd0741ac5" width="400" height="300" style="display: inline-block;" /> 
</p>

## Currently features/supports:
- 4 crypto tickers, from Binance Futures; BTCUSDT, ETHUSDT, SOLUSDT, LTCUSDT
- 1m, 3m, 5m, 15m and 30m timeframe selections, for both candlestick and footprint charts. <sup>Tick based "timeframe" selections is planned</sup>
- Tick size selections for grouping trade prices/quantities on footprint chart
- Size filtering for trades showing in time&sales table and heatmap chart
- **Only 20 bid/ask levels for the orderbook**
- **No historical data for trades/orderbook**

There is no server-side, nor is one needed yet. It all happens with exchange API fetch/websockets on the user end.

### How the heatmap works:
Each bid or ask is represented by single pixels. These pixels correspond to their price levels and ~100ms time intervals. The color opacity of each pixel indicates the quantity of the bid or ask. It is relative to all other bid or ask quantities visible within the graph time range.

## Build from source
If you want to compile it yourself,
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
~~Trading implementation is highly experimental; advised not to use any trading functionality with a real account~~ 
> Trading functionalities/pane is removed upon with the Iced master migration, as it was very much of unfinished. Still a WIP.
> 
<p align="center">
  <img src="https://github.com/akenshaw/iced-trade/assets/63060680/e7b55751-b547-4548-ac95-5348c6c60385" width="404,5" />
</p>

>  My goal was just to create a lightweight and minimal GUI for trading only. It then evolved to "trading on the chart", which later became a tool to track basic orderflow, as I've discovered on the way that it's not ready for full trading capabilities. Currently still trying to stabilize more charting stuff, to focus on utilizing trading on the charts later on.
> 
