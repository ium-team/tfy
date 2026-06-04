export function PriceCard17({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="17">{totalAmount * (1 + taxRate)}</section>;
}
