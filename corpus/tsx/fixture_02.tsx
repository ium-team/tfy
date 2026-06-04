type PriceCardProps2 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard2({ item, taxRate }: PriceCardProps2) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="2">{totalAmount * (1 + taxRate)}</section>;
}
