type CartItem18 = { price: number; qty?: number };
export function calculateDiscount18(cartItems: CartItem18[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 180 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook18 {
  lookupPrice(sku: string): number { return sku.length + 18; }
}
