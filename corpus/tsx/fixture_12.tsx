type PriceCardProps12 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard12({ item, taxRate }: PriceCardProps12) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="12">{totalAmount * (1 + taxRate)}</section>;
}
