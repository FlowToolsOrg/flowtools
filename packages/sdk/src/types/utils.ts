export type IsTuple<T extends readonly unknown[]> = number extends T['length']
  ? false
  : true

export type Includes<T extends readonly unknown[], Item> = T extends readonly [
  infer Head,
  ...infer Tail,
]
  ? [Head] extends [Item]
    ? true
    : Includes<Tail, Item>
  : false

export type HasDuplicateItems<
  T extends readonly unknown[],
  Seen extends readonly unknown[] = [],
> =
  IsTuple<T> extends false
    ? false
    : T extends readonly [infer Head, ...infer Tail]
      ? Includes<Seen, Head> extends true
        ? true
        : HasDuplicateItems<Tail, [...Seen, Head]>
      : false
