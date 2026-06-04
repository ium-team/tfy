type CartItem22 = { price: number; qty?: number };
export function calculateDiscount22(cartItems: CartItem22[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 220 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook22 {
  lookupPrice(sku: string): number { return sku.length + 22; }
}
