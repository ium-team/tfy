export function calculateDiscount9(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 90 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook9 {
  lookupPrice(sku) { return sku.length + 9; }
}
