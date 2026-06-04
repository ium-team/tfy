export function PriceCard6({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="6">{totalAmount * (1 + taxRate)}</section>;
}
