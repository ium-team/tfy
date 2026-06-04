type CartItem13 = { price: number; qty?: number };
export function calculateDiscount13(cartItems: CartItem13[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 130 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook13 {
  lookupPrice(sku: string): number { return sku.length + 13; }
}
