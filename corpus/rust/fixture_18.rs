pub struct PriceBook18 {
    pub multiplier: f64,
}

pub fn calculate_discount_18(cart_items: &[(f64, f64)], tax_rate: f64) -> f64 {
    let total_amount: f64 = cart_items.iter().map(|(price, qty)| price * qty).sum();
    if total_amount > 180.0 { total_amount * (1.0 + tax_rate) } else { total_amount }
}
