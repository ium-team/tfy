type PriceCardProps8 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard8({ item, taxRate }: PriceCardProps8) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="8">{totalAmount * (1 + taxRate)}</section>;
}
