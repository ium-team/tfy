type CartItem10 = { price: number; qty?: number };
export function calculateDiscount10(cartItems: CartItem10[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 100 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook10 {
  lookupPrice(sku: string): number { return sku.length + 10; }
}
