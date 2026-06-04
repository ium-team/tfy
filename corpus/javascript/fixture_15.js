export function calculateDiscount15(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 150 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook15 {
  lookupPrice(sku) { return sku.length + 15; }
}
