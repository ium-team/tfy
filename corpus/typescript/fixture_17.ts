type CartItem17 = { price: number; qty?: number };
export function calculateDiscount17(cartItems: CartItem17[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 170 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook17 {
  lookupPrice(sku: string): number { return sku.length + 17; }
}
