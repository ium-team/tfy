export function calculateDiscount14(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 140 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook14 {
  lookupPrice(sku) { return sku.length + 14; }
}
