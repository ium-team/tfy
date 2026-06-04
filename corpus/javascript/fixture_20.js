export function calculateDiscount20(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 200 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook20 {
  lookupPrice(sku) { return sku.length + 20; }
}
