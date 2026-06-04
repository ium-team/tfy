export function calculateDiscount13(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 130 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook13 {
  lookupPrice(sku) { return sku.length + 13; }
}
