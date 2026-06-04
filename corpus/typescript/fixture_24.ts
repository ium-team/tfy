type CartItem24 = { price: number; qty?: number };
export function calculateDiscount24(cartItems: CartItem24[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 240 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook24 {
  lookupPrice(sku: string): number { return sku.length + 24; }
}
