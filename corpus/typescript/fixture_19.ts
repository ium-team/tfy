type CartItem19 = { price: number; qty?: number };
export function calculateDiscount19(cartItems: CartItem19[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 190 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook19 {
  lookupPrice(sku: string): number { return sku.length + 19; }
}
