export function PriceCard13({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="13">{totalAmount * (1 + taxRate)}</section>;
}
