type CartItem7 = { price: number; qty?: number };
export function calculateDiscount7(cartItems: CartItem7[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 70 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook7 {
  lookupPrice(sku: string): number { return sku.length + 7; }
}
