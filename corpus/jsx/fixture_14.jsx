export function PriceCard14({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="14">{totalAmount * (1 + taxRate)}</section>;
}
