export function PriceCard9({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="9">{totalAmount * (1 + taxRate)}</section>;
}
