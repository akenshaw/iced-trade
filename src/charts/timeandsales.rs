use chrono::NaiveDateTime;
use iced::{
    alignment, Element, Length
};
use iced::widget::{Column, Row, Container, Text, container, Space};
use crate::{Message, style, data_providers::Trade};

struct ConvertedTrade {
    time: NaiveDateTime,
    price: f32,
    qty: f32,
    is_sell: bool,
}
pub struct TimeAndSales {
    recent_trades: Vec<ConvertedTrade>,
    size_filter: f32,
}
impl TimeAndSales {
    pub fn new() -> Self {
        Self {
            recent_trades: Vec::new(),
            size_filter: 0.0,
        }
    }
    pub fn set_size_filter(&mut self, value: f32) {
        self.size_filter = value;
    }

    pub fn update(&mut self, trades_buffer: &Vec<Trade>) {
        for trade in trades_buffer {
            let trade_time = NaiveDateTime::from_timestamp(trade.time / 1000, (trade.time % 1000) as u32 * 1_000_000);
            let converted_trade = ConvertedTrade {
                time: trade_time,
                price: trade.price,
                qty: trade.qty,
                is_sell: trade.is_sell,
            };
            self.recent_trades.push(converted_trade);
        }

        if self.recent_trades.len() > 2000 {
            let drain_to = self.recent_trades.len() - 2000;
            self.recent_trades.drain(0..drain_to);
        }
    }
    pub fn view(&self) -> Element<'_, Message> {
        let mut trades_column = Column::new()
            .height(Length::Fill)
            .padding(10);

        let filtered_trades: Vec<_> = self.recent_trades.iter().filter(|trade| (trade.qty*trade.price) >= self.size_filter).collect();

        let max_qty = filtered_trades.iter().map(|trade| trade.qty).fold(0.0, f32::max);
    
        if filtered_trades.is_empty() {
            trades_column = trades_column.push(
                Text::new("No trades")
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .size(16)
            );
        } else {
            for trade in filtered_trades.iter().rev().take(80) {
                let trade: &ConvertedTrade = trade;

                let trade_row = Row::new()
                    .push(
                        container(Text::new(format!("{}", trade.time.format("%M:%S.%3f"))).size(14))
                            .width(Length::FillPortion(8)).align_x(alignment::Horizontal::Center)
                    )
                    .push(
                        container(Text::new(format!("{}", trade.price)).size(14))
                            .width(Length::FillPortion(6))
                    )
                    .push(
                        container(Text::new(if trade.is_sell { "Sell" } else { "Buy" }).size(14))
                            .width(Length::FillPortion(4)).align_x(alignment::Horizontal::Left)
                    )
                    .push(
                        container(Text::new(format!("{}", trade.qty)).size(14))
                            .width(Length::FillPortion(4))
                    );

                let color_alpha = trade.qty / max_qty;
    
                trades_column = trades_column.push(container(trade_row)
                    .style( move |_| if trade.is_sell { style::sell_side_red(color_alpha) } else { style::buy_side_green(color_alpha) }));
    
                trades_column = trades_column.push(Container::new(Space::new(Length::Fixed(0.0), Length::Fixed(5.0))));
            }
        }
    
        trades_column.into()  
    }    
}