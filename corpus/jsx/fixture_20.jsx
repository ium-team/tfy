export function PriceCard20({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="20">{totalAmount * (1 + taxRate)}</section>;
}
