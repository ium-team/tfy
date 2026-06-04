export function PriceCard7({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="7">{totalAmount * (1 + taxRate)}</section>;
}
