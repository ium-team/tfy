export function calculateDiscount1(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 10 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook1 {
  lookupPrice(sku) { return sku.length + 1; }
}
