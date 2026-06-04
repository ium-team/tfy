type Props = { name: string };
export function UnicodeCard({ name }: Props) {
  // keep tsx comment
  const text: string = `hé 😀 ${name}`;
  return <div title="a : b">{text}</div>;
}
