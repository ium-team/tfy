type CartItem14 = { price: number; qty?: number };
export function calculateDiscount14(cartItems: CartItem14[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 140 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook14 {
  lookupPrice(sku: string): number { return sku.length + 14; }
}
