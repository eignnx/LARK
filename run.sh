set -e

# Get the filename without the extension
SRC=$(basename -s .meadow "$1")

# Compile
cargo run --bin meadowlark -- "../$1" > "../$SRC.lark"

cd ..

# Assemble
customasm "$SRC.lark"

# Run
cargo run --bin lark-ui -- $2 "../$SRC.bin"
