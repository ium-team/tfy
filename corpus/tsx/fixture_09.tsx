type PriceCardProps9 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard9({ item, taxRate }: PriceCardProps9) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="9">{totalAmount * (1 + taxRate)}</section>;
}
