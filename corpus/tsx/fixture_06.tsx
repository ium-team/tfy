type PriceCardProps6 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard6({ item, taxRate }: PriceCardProps6) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="6">{totalAmount * (1 + taxRate)}</section>;
}
