export function calculateDiscount17(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 170 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook17 {
  lookupPrice(sku) { return sku.length + 17; }
}
