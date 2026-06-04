type PriceCardProps16 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard16({ item, taxRate }: PriceCardProps16) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="16">{totalAmount * (1 + taxRate)}</section>;
}
