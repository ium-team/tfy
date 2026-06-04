type PriceCardProps3 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard3({ item, taxRate }: PriceCardProps3) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="3">{totalAmount * (1 + taxRate)}</section>;
}
