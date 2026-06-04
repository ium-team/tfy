type PriceCardProps25 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard25({ item, taxRate }: PriceCardProps25) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="25">{totalAmount * (1 + taxRate)}</section>;
}
