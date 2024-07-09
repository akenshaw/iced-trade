<p align="center">
  <img width="400" height="225" alt="v0.3" src="https://github.com/akenshaw/iced-trade/assets/63060680/6aa1b587-5659-48c4-af8c-bcfc81168a10"  >  
  <img alt="v0.3" src="https://github.com/akenshaw/iced-trade/assets/63060680/8a13536c-f39e-48a2-9d02-a49a8bc965a5" width="400" height="225" style="display: inline-block;" />
  <img src="https://github.com/akenshaw/iced-trade/assets/63060680/1517e033-e336-45ee-9418-79135e8a8cd2" width="400" height="225" style="display: inline-block;" /> 
</p>

### Currently features/supports:
- 4 crypto tickers, both from Binance and Bybit; BTCUSDT, ETHUSDT, SOLUSDT, LTCUSDT (perpetual swaps)
- 1m, 3m, 5m, 15m and 30m timeframe selections for candlestick and footprint charts. <sup>Tick based "timeframe" selections is planned</sup>
- Tick size selections for grouping trade prices/quantities on footprint chart
- Size filtering for trades showing in time&sales table and heatmap chart
- **No historical data for trades/orderbook**

##### There is no server-side yet. It all happens with exchange API fetch/websockets on the user end.

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
~~Trading implementation is highly experimental; advised not to use any trading functionality with a real account~~ 
> Trading functionalities/pane is removed upon with the Iced master migration, as it was very much of unfinished. Still a WIP.
> 
<p align="center">
  <img src="https://github.com/akenshaw/iced-trade/assets/63060680/e7b55751-b547-4548-ac95-5348c6c60385" width="404,5" />
</p>

>  My intention was just to create a lightweight and minimal GUI for trading only. This idea evolved to "trading on the chart", but then, I've discovered on the way that it wasn't ready for it, so it just became a tool to track basic orderflow. Currently still trying to stabilize charting stuff a bit more, so that fully utilized trading would be easier to implement later on.
