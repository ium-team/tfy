type PriceCardProps20 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard20({ item, taxRate }: PriceCardProps20) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="20">{totalAmount * (1 + taxRate)}</section>;
}
