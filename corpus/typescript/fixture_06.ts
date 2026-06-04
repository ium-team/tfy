type CartItem6 = { price: number; qty?: number };
export function calculateDiscount6(cartItems: CartItem6[], taxRate: number): number {
  const totalAmount = cartItems.reduce((sum, item) => sum + item.price * (item.qty ?? 1), 0);
  return totalAmount > 60 ? totalAmount * (1 + taxRate) : totalAmount;
}

export class PriceBook6 {
  lookupPrice(sku: string): number { return sku.length + 6; }
}
