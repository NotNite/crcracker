# crcracker

obliterate xiv shader hashes. shoutouts ny and winter

---

XIV shaders store hashed names, usually in the format of `g_{word one}{word two}`. this takes a word list and cracks those hashes, assuming that format.

it's speedy, because

- it's in :rocket: :crab: rust :crab: :rocket:
- it's multi-threaded (albeit poorly)
- it uses CRC Combination (sparkles)

## usage

make a word list text file, containing single words seperated by a newline (such as `Emissive`, `Color`, etc). make sure the names are capitalized properly (pascal case), or it won't work - see `pascalify.js` for a script to do that for you.

then make a hash list file, containing hashes seperated a newline (such as `38A64362`). you can use any case sensitivity or precede with `0x`.

then, Just Run It:

```shell
cargo run --release -- --word-list=words.txt --hash-list=hashes.txt --threads=8 --prefix "g_"
```

by default, hashes will be printed both immediately when found and when all possible values have been found. see the `--print-when-found` argument below if you only want to see the results when it's finished.

## arguments

- `-w, --word-list`: path to word list file
- `-h, --hash-list`: path to hash list file
- `-t, --threads`: number of threads to use (default: 1, *incredibly* slow)
- `-p, --prefix`: prefix to use for hashing (optional)
- `--print-progress`: whether to print progress to stdout (default: true)
- `--print-when-found`: whether to print immediately when a hash is found (default: true)
- `--print-every`: when to print progress (default: 10000000)

## collisions

crc32 is *complete dogshit* and you will find a bunch of collisions. i'm using [the scrabble dictionary](https://raw.githubusercontent.com/raun/Scrabble/master/words.txt) for testing and it fucking thought `g_EmissiveColor` was `g_PestoSanguine`. make sure to clean up your dict
