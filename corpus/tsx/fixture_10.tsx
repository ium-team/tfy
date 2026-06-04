type PriceCardProps10 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard10({ item, taxRate }: PriceCardProps10) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="10">{totalAmount * (1 + taxRate)}</section>;
}
