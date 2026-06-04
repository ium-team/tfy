export function calculateDiscount4(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 40 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook4 {
  lookupPrice(sku) { return sku.length + 4; }
}
