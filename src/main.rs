use clap::Parser;
use cloudflare_zlib_sys::crc32;
use cloudflare_zlib_sys::crc32_combine;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Parser, Clone)]
struct Args {
    #[clap(short, long)]
    word_list: PathBuf,

    #[clap(short, long)]
    hash_list: PathBuf,

    #[clap(short, long, default_value = "1")]
    threads: usize,

    #[clap(long, default_value = "true")]
    print_when_found: bool,

    #[clap(short, long)]
    prefix: Option<String>,
}

// borrow of partially moved deez. this sucks
#[derive(Clone)]
struct Settings {
    threads: usize,
    print_when_found: bool,
    prefix: Option<String>,
    words: Vec<String>,
}

#[derive(Clone)]
struct WordTable {
    all_str_to_crc: HashMap<String, u32>,
    our_str_to_crc: HashMap<String, u32>,
    max_word_len: usize,
}

fn xiv_crc32(s: &[u8]) -> u32 {
    unsafe { !(crc32(0xFFFFFFFF, s.as_ptr(), s.len() as u32) as u32) }
}

fn xiv_crc32_combine(a: u32, b: u32, b_len: usize) -> u32 {
    unsafe { crc32_combine(a as u64, b as u64, b_len as isize) as u32 }
}

fn bruteforce(
    target_hash: u32,
    word_table: WordTable,
    settings: Settings,
    tx: std::sync::mpsc::Sender<Option<String>>,
) {
    for (word_one, hash_one) in &word_table.our_str_to_crc {
        let mut lookup: HashMap<usize, u32> = HashMap::new();

        for len in 1..=word_table.max_word_len {
            let blank_hash = xiv_crc32_combine(*hash_one, 0, len);
            lookup.insert(len, blank_hash);
        }

        for (word_two, hash_two) in &word_table.all_str_to_crc {
            let blank_hash = lookup.get(&word_two.len()).unwrap();
            let combined_hash = blank_hash ^ hash_two;

            if combined_hash == target_hash {
                let msg = format!("{}{}", word_one, word_two);

                if settings.print_when_found {
                    println!("[match] {:x} = {}", target_hash, msg);
                }

                tx.send(Some(msg)).ok();
            }
        }
    }

    tx.send(None).ok();
}

fn bruteforce_threaded(target_hash: u32, settings: Settings) -> Vec<String> {
    let mut handles = vec![];
    let mut mpsc_rx = vec![];

    let mut cycles = 0;

    let str_to_crc: HashMap<String, u32> = settings
        .words
        .iter()
        .map(|s| (s.clone(), xiv_crc32(s.as_bytes())))
        .collect();
    let words_split = settings
        .words
        .chunks_exact(settings.words.len() / settings.threads)
        .collect::<Vec<_>>();

    let prefix = settings.prefix.clone().unwrap_or_default();
    let max_word_len = settings.words.iter().map(|s| s.len()).max().unwrap();

    (0..settings.threads).for_each(|i| {
        let our_words = words_split[i]
            .iter()
            .map(|s| format!("{}{}", prefix, s))
            .collect::<Vec<String>>();
        let our_str_to_crc: HashMap<String, u32> = our_words
            .iter()
            .map(|s| (s.clone(), xiv_crc32(s.as_bytes())))
            .collect();

        cycles += our_words.len() * settings.words.len();

        let (tx, rx) = std::sync::mpsc::channel();

        // .clone() makes me sad
        let settings = settings.clone();
        let word_table = WordTable {
            all_str_to_crc: str_to_crc.clone(),
            our_str_to_crc,
            max_word_len,
        };

        let handle = std::thread::spawn(move || {
            bruteforce(target_hash, word_table, settings, tx);
        });

        handles.push(handle);
        mpsc_rx.push(rx);
    });

    let mut ret_none = 0;
    let mut possible = vec![];

    loop {
        for rx in &mpsc_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Some(r) => {
                        possible.push(r);
                    }
                    None => {
                        ret_none += 1;
                    }
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
    let crc_g = 0xd168b105;
    let crc_emissivecolor = 0x900676f0;
    let crc_g_emissivecolor = 0x38a64362;

    let test_full = xiv_crc32(b"g_EmissiveColor");
    assert_eq!(test_full, crc_g_emissivecolor);

    let test_g = xiv_crc32(b"g_");
    assert_eq!(test_g, crc_g);

    let test_emissivecolor = xiv_crc32(b"EmissiveColor");
    assert_eq!(test_emissivecolor, crc_emissivecolor);

    let test_full = xiv_crc32_combine(test_g, test_emissivecolor, b"EmissiveColor".len());
    assert_eq!(test_full, crc_g_emissivecolor);
}

fn main() {
    let args = Args::parse();
    test();

    let wordlist = std::fs::read_to_string(args.word_list).unwrap();
    let hashes = std::fs::read_to_string(args.hash_list).unwrap();
    let hashes: Vec<&str> = hashes.split_whitespace().collect::<Vec<_>>();

    let mut words: Vec<String> = wordlist.split_whitespace().map(|s| s.to_string()).collect();
    words.sort();
    words.dedup();

    println!(
        "[bruteforce] starting bruteforce - {} hashes, {} words",
        hashes.len(),
        words.len()
    );

    let settings = Settings {
        threads: args.threads,
        print_when_found: args.print_when_found,
        prefix: args.prefix,
        words,
    };

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
