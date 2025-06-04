# txml

An XML parser. It's about ~200 SLOC but it:

- Doesn't parse DTDs and therefore doesn't support custom entities
- Doesn't validate DTDs, of course
- Doesn't allow for streaming - you have to read the whole file at once
- Doesn't reject all non-well-formed documents
- Doesn't have any dependencies
- Doesn't allocate, which is nice

This parser is not meant for any usecase where you're not certain that
the document is well-formed, because it reports errors by simply ending
the event stream early.

This parser may be useful for parsing machine-readable specifications that
use XML such as Wayland and Vulkan.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
