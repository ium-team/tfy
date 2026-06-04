export function calculateDiscount2(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 20 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook2 {
  lookupPrice(sku) { return sku.length + 2; }
}
