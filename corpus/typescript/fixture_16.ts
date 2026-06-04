type CartItem16 = { price: number; qty?: number };
export function calculateDiscount16(cartItems: CartItem16[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 160 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook16 {
  lookupPrice(sku: string): number { return sku.length + 16; }
}
