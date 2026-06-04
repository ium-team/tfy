export function calculateDiscount24(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 240 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook24 {
  lookupPrice(sku) { return sku.length + 24; }
}
