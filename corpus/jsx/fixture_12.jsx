export function PriceCard12({ item, taxRate }) {
  const totalAmount = item.price * (item.qty ?? 1);
  return <section data-id="12">{totalAmount * (1 + taxRate)}</section>;
}
