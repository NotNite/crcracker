mod common;
mod crack_concat;
mod crack_xor;
mod xivcrc32;

use clap::Parser;
use common::{Args, Settings};
use xivcrc32::XivCrc32;

fn self_test() {
    // found with xiv_crc32(str.as_bytes())
    let crc_g = XivCrc32::new(0xD168B105, 2);
    let crc_emissivecolor = XivCrc32::new(0x900676F0, 13);
    let crc_g_emissivecolor = XivCrc32::new(0x38A64362, 15);

    let test_full = XivCrc32::from(b"g_EmissiveColor");
    assert_eq!(test_full, crc_g_emissivecolor);

    let test_g = XivCrc32::from(b"g_");
    assert_eq!(test_g, crc_g);

    let test_emissivecolor = XivCrc32::from(b"EmissiveColor");
    assert_eq!(test_emissivecolor, crc_emissivecolor);

    let test_full = test_g + test_emissivecolor;
    assert_eq!(test_full, crc_g_emissivecolor);
}

fn main() {
    self_test();

    let args = Args::parse();

    if args.xor {
        main_xor(args);
    } else {
        main_concat(args);
    }
}

fn main_concat(args: Args) {
    if args.words < 1 || args.words > 3 {
        panic!("--words must be 1, 2 or 3");
    }

    let hashes = args.read_hashes();
    let settings = Settings::from(&args);

    println!(
        "using prefix \"{}\" ({:x}), separator \"{}\" ({:x}), suffix \"{}\" ({:x})",
        settings.prefix,
        settings.prefix_hash.crc,
        settings.separator,
        settings.separator_hash.crc,
        settings.suffix,
        settings.suffix_hash.crc,
    );

    println!(
        "[bruteforce] starting bruteforce - {} hashes, {} words, {} words max",
        hashes.len(),
        settings.words.len(),
        settings.max_words,
    );

    for hash in hashes {
        let result = crack_concat::bruteforce_threaded(hash, &settings);
        if !result.is_empty() {
            let items = result.join(", ");
            println!("[result] {:x} = {}", hash, items);
        } else {
            println!("[result] {:x} = unknown", hash);
        }
    }
}

fn main_xor(args: Args) {
    let hashes = args.read_hashes();
    let settings = Settings::from(&args);

    println!(
        "using suffix \"{}\" ({:x})",
        settings.suffix, settings.suffix_hash.crc,
    );

    println!(
        "[bruteforce] starting bruteforce - {} hashes, {} words, xor mode",
        hashes.len(),
        settings.words.len(),
    );

    for hash in hashes {
        let result = crack_xor::bruteforce_threaded(hash, &settings);
        if !result.is_empty() {
            let items = result
                .iter()
                .map(|(word1, word2)| format!("{} / {}", word1, word2))
                .collect::<Vec<_>>()
                .join(", ");
            println!("[result] {:x} = {}", hash, items);
        } else {
            println!("[result] {:x} = unknown", hash);
        }
    }
}
