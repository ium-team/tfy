export function PriceCard21({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="21">{totalAmount * (1 + taxRate)}</section>;
}
