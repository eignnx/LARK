set -e

# Get the filename without the extension
SRC=$(basename -s .meadow "$1")

# Compile
cd ./meadowlark
time cargo run "../$1" > "../$SRC.lark"

cd ..

# Assemble
time customasm "$SRC.lark"

# Run
cd ./lark-vm
time cargo run $2 "../$SRC.bin"
