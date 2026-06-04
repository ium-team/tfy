function Card(props: { name: string }) {
  const title = `hello : ${props.name}`;
  return <div>{title}</div>;
}
