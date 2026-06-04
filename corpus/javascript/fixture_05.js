export function calculateDiscount5(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 50 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook5 {
  lookupPrice(sku) { return sku.length + 5; }
}
