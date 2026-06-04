export function PriceCard10({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="10">{totalAmount * (1 + taxRate)}</section>;
}
