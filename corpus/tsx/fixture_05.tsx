type PriceCardProps5 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard5({ item, taxRate }: PriceCardProps5) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="5">{totalAmount * (1 + taxRate)}</section>;
}
