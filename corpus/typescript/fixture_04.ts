type CartItem4 = { price: number; qty?: number };
export function calculateDiscount4(cartItems: CartItem4[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 40 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook4 {
  lookupPrice(sku: string): number { return sku.length + 4; }
}
