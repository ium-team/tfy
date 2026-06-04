export function calculateDiscount6(cartItems, taxRate) {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 60 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook6 {
  lookupPrice(sku) { return sku.length + 6; }
}
