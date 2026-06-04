type CartItem11 = { price: number; qty?: number };
export function calculateDiscount11(cartItems: CartItem11[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 110 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook11 {
  lookupPrice(sku: string): number { return sku.length + 11; }
}
