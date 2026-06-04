type PriceCardProps11 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard11({ item, taxRate }: PriceCardProps11) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="11">{totalAmount * (1 + taxRate)}</section>;
}
