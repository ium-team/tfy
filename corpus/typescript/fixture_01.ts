type CartItem1 = { price: number; qty?: number };
export function calculateDiscount1(cartItems: CartItem1[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 10 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook1 {
  lookupPrice(sku: string): number { return sku.length + 1; }
}
