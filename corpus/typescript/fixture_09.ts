type CartItem9 = { price: number; qty?: number };
export function calculateDiscount9(cartItems: CartItem9[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 90 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook9 {
  lookupPrice(sku: string): number { return sku.length + 9; }
}
