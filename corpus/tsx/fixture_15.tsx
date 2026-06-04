type PriceCardProps15 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard15({ item, taxRate }: PriceCardProps15) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="15">{totalAmount * (1 + taxRate)}</section>;
}
