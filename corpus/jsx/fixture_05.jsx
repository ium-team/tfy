export function PriceCard5({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="5">{totalAmount * (1 + taxRate)}</section>;
}
