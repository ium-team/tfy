export function PriceCard4({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="4">{totalAmount * (1 + taxRate)}</section>;
}
