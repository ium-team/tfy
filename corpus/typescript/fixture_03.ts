type CartItem3 = { price: number; qty?: number };
export function calculateDiscount3(cartItems: CartItem3[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 30 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook3 {
  lookupPrice(sku: string): number { return sku.length + 3; }
}
