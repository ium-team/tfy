export function calculateDiscount12(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 120 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook12 {
  lookupPrice(sku) { return sku.length + 12; }
}
