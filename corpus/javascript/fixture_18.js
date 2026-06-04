export function calculateDiscount18(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 180 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook18 {
  lookupPrice(sku) { return sku.length + 18; }
}
