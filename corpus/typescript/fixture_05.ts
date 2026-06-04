type CartItem5 = { price: number; qty?: number };
export function calculateDiscount5(cartItems: CartItem5[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 50 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook5 {
  lookupPrice(sku: string): number { return sku.length + 5; }
}
