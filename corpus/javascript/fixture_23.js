export function calculateDiscount23(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 230 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook23 {
  lookupPrice(sku) { return sku.length + 23; }
}
