export function PriceCard23({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="23">{totalAmount * (1 + taxRate)}</section>;
}
