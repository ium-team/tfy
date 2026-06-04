export function calculateDiscount16(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 160 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook16 {
  lookupPrice(sku) { return sku.length + 16; }
}
