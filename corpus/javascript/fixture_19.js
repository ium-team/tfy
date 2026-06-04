export function calculateDiscount19(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 190 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook19 {
  lookupPrice(sku) { return sku.length + 19; }
}
