export function calculateDiscount25(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 250 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook25 {
  lookupPrice(sku) { return sku.length + 25; }
}
