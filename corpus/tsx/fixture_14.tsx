type PriceCardProps14 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard14({ item, taxRate }: PriceCardProps14) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="14">{totalAmount * (1 + taxRate)}</section>;
}
