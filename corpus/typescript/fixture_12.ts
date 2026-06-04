type CartItem12 = { price: number; qty?: number };
export function calculateDiscount12(cartItems: CartItem12[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 120 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook12 {
  lookupPrice(sku: string): number { return sku.length + 12; }
}
