export function calculateDiscount22(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 220 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook22 {
  lookupPrice(sku) { return sku.length + 22; }
}
