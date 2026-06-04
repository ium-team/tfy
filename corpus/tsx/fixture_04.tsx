type PriceCardProps4 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard4({ item, taxRate }: PriceCardProps4) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="4">{totalAmount * (1 + taxRate)}</section>;
}
