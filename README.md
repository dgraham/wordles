# Wordles

A Wordle solver.

## Usage

```
$ wordles -h
A Wordle solver

Usage: wordles [options]

Options:
    -c, --contains L3   List of contains rules
    -m, --match S1,E5   List of match rules
    -n, --none R,N      List of none rules
    -o, --once E        List of once rules
        --no-cache      Rank without reading or writing the cache
        --cache DIR     Read or create the cache at this path
        --dict FILE     Read words from dictionary file
        --limit NUM     Limit output words (default: 5)
        --no-limit      Print all candidate words
        --frequency     Print character frequencies
        --patterns      Print sample patterns
        --words         Print built-in dictionary
    -v, --verbose       Print ranked words with scores
    -V, --version       Print version information
    -h, --help          Print this help message
```

## Development

```
cargo test
cargo build --release
```

## License

Distributed under the MIT License. See the LICENSE file for details.
