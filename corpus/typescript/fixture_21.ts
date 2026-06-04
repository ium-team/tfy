type CartItem21 = { price: number; qty?: number };
export function calculateDiscount21(cartItems: CartItem21[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 210 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook21 {
  lookupPrice(sku: string): number { return sku.length + 21; }
}
