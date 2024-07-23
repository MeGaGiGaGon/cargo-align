# cargo-align

A simple tool for aligning text in code.

# Installation

`cargo install cargo-align`

# Usage

In the crate you want to align run `cargo align`

Writing the string `align_by stop` anywhere in a file will make the rest of the file be skipped.

Writing the string `align_by ""` will have the contents of the following lines aligned, until the first line that doesn't match the quote contents.

The matching aligned lines can be sorted after alignment by writing `align_by sort ""`.

The alignment markers are space seperated, `align_by "= ;"` will first align by `=`, then by `;`, left to right. 

Alignment markers are only used once. This means `align_by "="` will only align on the first found `=` per line, and ignore subsequent ones.

Double quotes can be aligned on using an escaping `\`. `align_by "\""`

# Limitations, Rationale, and Current State

This tool has been developed primarilly for my own projects. It currently has just enough features to support my use cases. Issues/pull requests are welcome if you would like to see it support yours.

Currently it is hardcoded to use the workspace path of `cargo metadata` for ease of implementation.

I chose the `align_by sort ""` syntax since outside of comments and strings it is invalid Rust and TOML syntax, making conflicts with existing code highly unlikely. Since there is no special checks for if the alignment statement is inside a Rust/TOML comment, it will work on any programming language.

It is inspired by the VSCode extention [`align-by-regex`](https://marketplace.visualstudio.com/items?itemName=janjoerke.align-by-regex), though as of now the regex part has been dropped for ease of implementation.

Thanks to the extremely basic implementation I had to add `align_by stop` otherwise it's almost impossible to write about/test.

# Examples

Unaligned
```rust
align_by "= ;"
let a = 111;
let bbb = 2;
```

Aligned

```rust
align_by "= ;"
let a   = 111;
let bbb =   2;
```

More examples can be found in the tests.

## Without `cargo install`

`git clone https://github.com/MeGaGiGaGon/cargo-align.git`

`cd cargo-align`

`cargo build --release`

In the crate you want to align run `path_you_cloned_to/target/release/cargo-align.exe`

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
