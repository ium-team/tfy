export function PriceCard19({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="19">{totalAmount * (1 + taxRate)}</section>;
}
