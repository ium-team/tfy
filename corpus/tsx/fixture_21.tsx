type PriceCardProps21 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard21({ item, taxRate }: PriceCardProps21) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="21">{totalAmount * (1 + taxRate)}</section>;
}
