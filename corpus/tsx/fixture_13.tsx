type PriceCardProps13 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard13({ item, taxRate }: PriceCardProps13) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="13">{totalAmount * (1 + taxRate)}</section>;
}
