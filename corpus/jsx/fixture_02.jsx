export function PriceCard2({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="2">{totalAmount * (1 + taxRate)}</section>;
}
