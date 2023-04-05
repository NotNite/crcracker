use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;

mod xivcrc32;

use xivcrc32::XivCrc32;

#[derive(Parser, Clone)]
struct Args {
    #[clap(short = 'W', long)]
    word_list: PathBuf,

    #[clap(short = 'H', long)]
    hash_list: PathBuf,

    #[clap(short = 't', long, default_value = "1")]
    threads: usize,

    #[clap(short = 'w', long, default_value = "2")]
    words: usize,

    #[clap(long, default_value = "true")]
    print_when_found: bool,

    #[clap(short = 'p', long)]
    prefix: Option<String>,

    #[clap(short = 'P', long)]
    prefix_hash: Option<String>,

    #[clap(short = 's', long)]
    separator: Option<String>,

    #[clap(short = 'x', long)]
    suffix: Option<String>,
}

// borrow of partially moved deez. this sucks
#[derive(Clone)]
struct Settings {
    threads: usize,
    print_when_found: bool,
    prefix: String,
    prefix_hash: XivCrc32,
    separator: String,
    separator_hash: XivCrc32,
    suffix: String,
    suffix_hash: XivCrc32,
    max_words: usize,
    words: Vec<String>,
}

#[derive(Clone)]
struct WordTable {
    bare_str_to_crc: Vec<HashMap<String, u32>>,
    suffixed_crc_to_str: Vec<HashMap<u32, String>>,
    prefixed_str_to_crc: HashMap<String, XivCrc32>,
}

fn bruteforce(
    target_hash: u32,
    word_table: WordTable,
    settings: Settings,
    tx: std::sync::mpsc::Sender<Option<String>>,
) {
    for (word_one, hash_one) in &word_table.prefixed_str_to_crc {
        if (*hash_one + settings.suffix_hash).crc == target_hash {
            let msg = format!("{}{}{}", settings.prefix, word_one, settings.suffix);

            if settings.print_when_found {
                println!("[match] {:x} = {}", target_hash, msg);
            }

            tx.send(Some(msg)).unwrap();
        }

        if settings.max_words == 2 {
            let mut blank_hash = *hash_one + settings.separator_hash;
            for suffixed_crc_to_str in &word_table.suffixed_crc_to_str {
                blank_hash += XivCrc32::zero(1);
                let hash_two = blank_hash.crc ^ target_hash;
                if let Some(word_two) = suffixed_crc_to_str.get(&hash_two) {
                    let msg = format!("{}{}{}{}{}", settings.prefix, word_one, settings.separator, word_two, settings.suffix);

                    if settings.print_when_found {
                        println!("[match] {:x} = {}", target_hash, msg);
                    }

                    tx.send(Some(msg)).unwrap();
                }
            }
        } else if settings.max_words >= 3 {
            let mut blank_hash = *hash_one;
            for bare_str_to_crc in &word_table.bare_str_to_crc {
                blank_hash += XivCrc32::zero(1);
                for (word_two, hash_two) in bare_str_to_crc {
                    let combined_hash = blank_hash ^ XivCrc32::new(*hash_two, 0);
                    if (combined_hash + settings.suffix_hash).crc == target_hash {
                        let msg = format!("{}{}{}{}{}", settings.prefix, word_one, settings.separator, word_two, settings.suffix);

                        if settings.print_when_found {
                            println!("[match] {:x} = {}", target_hash, msg);
                        }

                        tx.send(Some(msg)).unwrap();
                    }

                    let mut blank_hash_two = combined_hash + settings.separator_hash;
                    for suffixed_crc_to_str in &word_table.suffixed_crc_to_str {
                        blank_hash_two += XivCrc32::zero(1);
                        let hash_three = blank_hash_two.crc ^ target_hash;
                        if let Some(word_three) = suffixed_crc_to_str.get(&hash_three) {
                            let msg = format!("{}{}{}{}{}{}{}", settings.prefix, word_one, settings.separator, word_two, settings.separator, word_three, settings.suffix);

                            if settings.print_when_found {
                                println!("[match] {:x} = {}", target_hash, msg);
                            }

                            tx.send(Some(msg)).unwrap();
                        }
                    }
                }
            }
        }
    }

    tx.send(None).unwrap();
}

fn bruteforce_threaded(target_hash: u32, settings: Settings) -> Vec<String> {
    let mut handles = vec![];
    let (tx, rx) = std::sync::mpsc::channel();

    let all_str_to_crc: HashMap<String, XivCrc32> = settings
        .words
        .iter()
        .map(|s| (s.clone(), XivCrc32::from(&s[..])))
        .collect();
    let words_split = settings
        .words
        .chunks_exact(settings.words.len() / settings.threads)
        .collect::<Vec<_>>();

    let (bare_str_to_crc, suffixed_crc_to_str) = {
        let mut bare_str_to_crc_builder: Vec<HashMap<String, u32>> = Vec::new();
        let mut suffixed_crc_to_str_builder: Vec<HashMap<u32, String>> = Vec::new();
        for (word, hash) in &all_str_to_crc {
            while bare_str_to_crc_builder.len() < word.len() {
                bare_str_to_crc_builder.push(HashMap::new());
            }
            while suffixed_crc_to_str_builder.len() < word.len() + settings.suffix_hash.len {
                suffixed_crc_to_str_builder.push(HashMap::new());
            }
            bare_str_to_crc_builder[word.len() - 1].insert(word.to_owned(), hash.crc);
            if let Some(other_word) = suffixed_crc_to_str_builder[word.len() + settings.suffix_hash.len - 1].insert((*hash + settings.suffix_hash).crc, word.to_owned()) {
                println!("Collision between words {} and {}", other_word, word);
            }
        }
        (bare_str_to_crc_builder, suffixed_crc_to_str_builder)
    };

    (0..settings.threads).for_each(|i| {
        let our_words = Vec::from(words_split[i]);
        let our_prefixed_str_to_crc: HashMap<String, XivCrc32> = our_words
            .iter()
            .map(|s| (s.clone(), settings.prefix_hash + XivCrc32::from(&s[..])))
            .collect();

        let worker_tx = tx.clone();

        // .clone() makes me sad
        let settings = settings.clone();
        let word_table = WordTable {
            bare_str_to_crc: bare_str_to_crc.clone(),
            suffixed_crc_to_str: suffixed_crc_to_str.clone(),
            prefixed_str_to_crc: our_prefixed_str_to_crc,
        };

        let handle = std::thread::spawn(move || {
            bruteforce(target_hash, word_table, settings, worker_tx);
        });

        handles.push(handle);
    });

    let mut ret_none = 0;
    let mut possible = vec![];

    loop {
        if let Ok(result) = rx.recv() {
            match result {
                Some(r) => {
                    possible.push(r);
                }
                None => {
                    ret_none += 1;
                }
            }
        }

        if ret_none == settings.threads {
            return possible;
        }
    }
}

fn test() {
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
    let args = Args::parse();
    test();

    if args.words < 1 || args.words > 3 {
        panic!("--words must be 1, 2 or 3");
    }

    let wordlist = std::fs::read_to_string(args.word_list).unwrap();
    let hashes = std::fs::read_to_string(args.hash_list).unwrap();
    let hashes: Vec<&str> = hashes.split_whitespace().collect::<Vec<_>>();

    let mut words: Vec<String> = wordlist.split_whitespace().map(|s| s.to_string()).collect();
    words.sort();
    words.dedup();

    let (prefix, prefix_hash) = match &args.prefix {
        Some(s) => match &args.prefix_hash {
            Some(hs) => (format!("…{}", s), XivCrc32::new(u32::from_str_radix(hs, 16).unwrap(), 0) + XivCrc32::from(&s[..])),
            None => (s.clone(), XivCrc32::from(&s[..])),
        },
        None => match &args.prefix_hash {
            Some(hs) => ("…".to_string(), XivCrc32::new(u32::from_str_radix(hs, 16).unwrap(), 0)),
            None => (String::new(), XivCrc32::default()),
        },
    };

    let (separator, separator_hash) = match &args.separator {
        Some(s) => (s.clone(), XivCrc32::from(&s[..])),
        None => (String::new(), XivCrc32::default()),
    };

    let (suffix, suffix_hash) = match &args.suffix {
        Some(s) => (s.clone(), XivCrc32::from(&s[..])),
        None => (String::new(), XivCrc32::default()),
    };

    let settings = Settings {
        threads: args.threads,
        print_when_found: args.print_when_found,
        prefix,
        prefix_hash,
        separator,
        separator_hash,
        suffix,
        suffix_hash,
        max_words: args.words,
        words,
    };

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
        let mut hash = hash;
        if hash.starts_with("0x") {
            hash = &hash[2..];
        }
        let hash = u32::from_str_radix(hash, 16).unwrap();

        let result = bruteforce_threaded(hash, settings.clone());
        if !result.is_empty() {
            let items = result.join(", ");
            println!("[result] {:x} = {}", hash, items);
        } else {
            println!("[result] {:x} = unknown", hash);
        }
    }
}
