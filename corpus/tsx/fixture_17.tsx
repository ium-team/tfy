type PriceCardProps17 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard17({ item, taxRate }: PriceCardProps17) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="17">{totalAmount * (1 + taxRate)}</section>;
}
