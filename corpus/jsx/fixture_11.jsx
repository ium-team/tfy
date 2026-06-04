export function PriceCard11({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="11">{totalAmount * (1 + taxRate)}</section>;
}
