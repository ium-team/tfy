type CartItem8 = { price: number; qty?: number };
export function calculateDiscount8(cartItems: CartItem8[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 80 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook8 {
  lookupPrice(sku: string): number { return sku.length + 8; }
}
