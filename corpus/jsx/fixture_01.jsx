export function PriceCard1({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="1">{totalAmount * (1 + taxRate)}</section>;
}
