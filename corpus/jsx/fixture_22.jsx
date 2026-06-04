export function PriceCard22({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="22">{totalAmount * (1 + taxRate)}</section>;
}
