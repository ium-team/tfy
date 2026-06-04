export function PriceCard16({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="16">{totalAmount * (1 + taxRate)}</section>;
}
