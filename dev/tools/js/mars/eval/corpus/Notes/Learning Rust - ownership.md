# Learning Rust: ownership and borrowing

Every value has one owner. When the owner goes out of scope, the value is dropped.

```rust
fn main() {
    let s = String::from("hello");
    let t = s;            // s moves into t; s can no longer be used
    println!("{t}");
}
```

## Borrowing

A function can borrow a value instead of taking it: `&T` for reading, `&mut T` for changing.
At any time there is either one mutable borrow or any number of shared borrows, never both.
The compiler checks this, so data races cannot compile.

```rust
fn push_twice(v: &mut Vec<i32>, x: i32) {
    v.push(x);
    v.push(x);
}
```

## Lifetimes

A reference may not outlive the value it points to. Most of the time the compiler infers
lifetimes; a function that returns a reference into one of two inputs needs `'a` to say which.

## When I got stuck

- Storing a reference inside a struct: easier to store an owned `String`, or an index.
- Two mutable borrows of one vector: split it with `split_at_mut`.

<!-- su-comments
[
  {
    "id": "9d4c1e72",
    "quote": "easier to store an owned",
    "comments": [
      { "id": "b5e08f3a", "author": "Casey", "at": "2026-05-15T21:00:00.000Z", "body": "Or use Rc and RefCell when several parts of the program must share and change the same data." }
    ]
  }
]
-->
