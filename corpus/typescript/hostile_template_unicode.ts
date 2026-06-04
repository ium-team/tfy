export function unicodeTemplate(name: string): string {
  // keep ts comment
  const text: string = `hé 😀 ${name}`;
  return text + " : spaced";
}
