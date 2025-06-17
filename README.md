# txml

An XML parser. It's small, but it:
- Doesn't parse or validate DTDs
- Doesn't support custom entities
- Requires the full document to be loaded in memory
- Accepts some non-well-formed documents
- Doesn't have any dependencies
- Doesn't allocate

This parser is not meant for usecases where you'd like good error messages
or perfect XML compliance. It's best used when communicating with a known
system, or when parsing existing, known documents written by hand.

## License

Licensed under either Apache-2.0 or MIT, at your option.
