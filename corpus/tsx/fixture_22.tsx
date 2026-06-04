type PriceCardProps22 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard22({ item, taxRate }: PriceCardProps22) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="22">{totalAmount * (1 + taxRate)}</section>;
}
