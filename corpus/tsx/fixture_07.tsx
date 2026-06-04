type PriceCardProps7 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard7({ item, taxRate }: PriceCardProps7) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="7">{totalAmount * (1 + taxRate)}</section>;
}
