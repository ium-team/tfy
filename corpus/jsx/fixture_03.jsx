export function PriceCard3({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="3">{totalAmount * (1 + taxRate)}</section>;
}
