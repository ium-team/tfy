export function PriceCard25({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="25">{totalAmount * (1 + taxRate)}</section>;
}
