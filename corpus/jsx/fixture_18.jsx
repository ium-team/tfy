export function PriceCard18({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="18">{totalAmount * (1 + taxRate)}</section>;
}
