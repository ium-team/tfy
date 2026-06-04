export function calculateDiscount10(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 100 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook10 {
  lookupPrice(sku) { return sku.length + 10; }
}
