export function PriceCard24({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="24">{totalAmount * (1 + taxRate)}</section>;
}
