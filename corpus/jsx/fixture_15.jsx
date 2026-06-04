export function PriceCard15({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="15">{totalAmount * (1 + taxRate)}</section>;
}
