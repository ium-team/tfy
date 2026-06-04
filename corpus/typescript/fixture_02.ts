type CartItem2 = { price: number; qty?: number };
export function calculateDiscount2(cartItems: CartItem2[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 20 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook2 {
  lookupPrice(sku: string): number { return sku.length + 2; }
}
