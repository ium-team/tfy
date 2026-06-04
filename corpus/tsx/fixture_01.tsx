type PriceCardProps1 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard1({ item, taxRate }: PriceCardProps1) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="1">{totalAmount * (1 + taxRate)}</section>;
}
