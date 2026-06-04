export function PriceCard8({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="8">{totalAmount * (1 + taxRate)}</section>;
}
