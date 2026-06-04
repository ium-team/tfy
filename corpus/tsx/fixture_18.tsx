type PriceCardProps18 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard18({ item, taxRate }: PriceCardProps18) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="18">{totalAmount * (1 + taxRate)}</section>;
}
