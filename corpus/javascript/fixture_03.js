export function calculateDiscount3(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 30 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook3 {
  lookupPrice(sku) { return sku.length + 3; }
}
