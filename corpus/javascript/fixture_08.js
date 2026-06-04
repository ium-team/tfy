export function calculateDiscount8(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 80 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook8 {
  lookupPrice(sku) { return sku.length + 8; }
}
