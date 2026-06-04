type PriceCardProps19 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard19({ item, taxRate }: PriceCardProps19) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="19">{totalAmount * (1 + taxRate)}</section>;
}
