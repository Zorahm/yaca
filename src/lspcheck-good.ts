/** An amount in minor units of some currency. */
export class Money {
  constructor(
    public amount: number,
    public currency: string = "EUR",
  ) {}

  /** A new amount, `factor` times this one. */
  times(factor: number): Money {
    return new Money(this.amount * factor, this.currency);
  }

  doubled(): Money {
    return this.times(LSP_CHECK_FACTOR);
  }
}

export const LSP_CHECK_FACTOR = 2;

export function total(prices: Money[]): number {
  return prices.reduce((sum, price) => sum + price.amount, 0);
}

const prices = [new Money(100), new Money(250)];
console.log(total(prices), prices[0].doubled().amount);
