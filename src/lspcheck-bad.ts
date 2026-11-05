// Deliberately wrong: a server should report each of these.

import { Money, total } from "./lspcheck-good";

function receipt(price: Money, count: string): string {
  const subtotal = price.times(count);
  const name: number = "receipt";
  return subtotal.amount + name;
}

const items = [new Money(100), new Money("250")];

export function printReceipt(): void {
  console.log(receipt(items[0], "3"), total(items), (window as any).nothingHere);
}
