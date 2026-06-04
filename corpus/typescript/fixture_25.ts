type CartItem25 = { price: number; qty?: number };
export function calculateDiscount25(cartItems: CartItem25[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 250 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook25 {
  lookupPrice(sku: string): number { return sku.length + 25; }
}
