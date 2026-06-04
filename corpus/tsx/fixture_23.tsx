type PriceCardProps23 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard23({ item, taxRate }: PriceCardProps23) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="23">{totalAmount * (1 + taxRate)}</section>;
}
