type PriceCardProps24 = { item: { price: number; qty?: number }; taxRate: number };
export function PriceCard24({ item, taxRate }: PriceCardProps24) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="24">{totalAmount * (1 + taxRate)}</section>;
}
