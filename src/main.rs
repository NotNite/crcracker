use clap::Parser;
use cloudflare_zlib_sys::crc32;
use cloudflare_zlib_sys::crc32_combine;
use std::collections::HashMap;
use std::path::PathBuf;

// found with xiv_crc32(str.as_bytes())
const CRC_G: u32 = 0xd168b105;
const CRC_EMISSIVECOLOR: u32 = 0x900676f0;
const CRC_G_EMISSIVECOLOR: u32 = 0x38a64362;

fn xiv_crc32(s: &[u8]) -> u32 {
    unsafe { !(crc32(0xFFFFFFFF, s.as_ptr(), s.len() as u32) as u32) }
}

fn xiv_crc32_combine(a: u32, b: u32, b_len: usize) -> u32 {
    unsafe { crc32_combine(a as u64, b as u64, b_len as isize) as u32 }
}

#[derive(Clone)]
struct WordTable {
    str_to_crc: HashMap<String, u32>,
    len_crc_map: HashMap<(usize, u32), u32>,
}

#[derive(Debug)]
enum Message {
    Progress(usize),
    Result(Option<String>),
}

fn bruteforce(
    target_hash: u32,
    word_table: &WordTable,
    full_words: &Vec<String>,
    our_words: &Vec<String>,
    tx: std::sync::mpsc::Sender<Message>,
    settings: Settings,
) {
    let mut current_cycle = 0;
    let mut last_cycle = 0;

    let print_when_found = settings.print_when_found;
    let print_every = settings.print_every;

    for word_one in our_words {
        for word_two in full_words {
            current_cycle += 1;
            if current_cycle % print_every == 0 {
                let diff = current_cycle - last_cycle;
                tx.send(Message::Progress(diff)).ok();
                last_cycle = current_cycle;
            }

            // I'm sleep deprived so I probably misunderstood the speedup here
            // UPDATE: <Ny> yes you did, but I'll get back at it when I'm done with what I'm currently on
            let crc_one = word_table.str_to_crc[word_one];
            let crc_two = word_table.str_to_crc[word_two];
            let len = word_two.len();
            let magical_hash = &word_table.len_crc_map[&(len, crc_one)];
            let magical_hash_two = magical_hash ^ crc_two;

            if magical_hash_two == target_hash {
                let msg = format!("g_{}{}", word_one, word_two);
                if print_when_found {
                    println!("[match] {:x} = {}", target_hash, msg);
                }
                tx.send(Message::Result(Some(msg))).ok();
            }
        }
    }

    tx.send(Message::Progress(current_cycle - last_cycle)).ok();
    tx.send(Message::Result(None)).ok();
}

fn bruteforce_threaded(
    target_hash: u32,
    word_table: &WordTable,
    words: &[Vec<String>],
    settings: Settings,
) -> Vec<String> {
    let mut handles = vec![];
    let mut mpsc_rx = vec![];

    let words_flat = words.iter().flatten().cloned().collect::<Vec<_>>();
    let mut cycles = 0;

    (0..settings.threads).for_each(|i| {
        // .clone() makes me sad but i am tired
        let our_words = words[i].clone();
        let words_flat = words_flat.clone();
        let word_table = word_table.clone();

        let (tx, rx) = std::sync::mpsc::channel();

        cycles += our_words.len() * words_flat.len();

        let settings = settings.clone();
        let handle = std::thread::spawn(move || {
            bruteforce(
                target_hash,
                &word_table,
                &words_flat,
                &our_words,
                tx,
                settings,
            );
        });

        handles.push(handle);
        mpsc_rx.push(rx);
    });

    let mut ret_none = 0;
    let mut progress: usize = 0;
    let mut last_progress = 0;

    if settings.print_progress {
        println!("[progress] 0/{} (0.00%)", cycles);
    }

    let mut possible = vec![];

    loop {
        for rx in &mpsc_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Message::Progress(p) => {
                        progress += p;
                        let diff = progress - last_progress;
                        if diff >= settings.print_every {
                            let clean_progress = progress - (progress % settings.print_every);
                            let percent = (clean_progress as f64 / cycles as f64) * 100.0;
                            if settings.print_progress {
                                println!(
                                    "[progress] {}/{} ({:.2}%)",
                                    clean_progress, cycles, percent
                                );
                            }
                            last_progress = clean_progress;
                        }
                    }
                    Message::Result(r) => {
                        if let Some(r) = r {
                            possible.push(r);
                        } else {
                            ret_none += 1;
                        }
                    }
                }
            }
        }

        if ret_none == settings.threads {
            return possible;
        }
    }
}

fn gen_words(words: Vec<String>, settings: Settings) -> (WordTable, Vec<Vec<String>>) {
    let mut str_to_crc = HashMap::new();
    let mut len_crc_map = HashMap::new();

    let max_word_len = words.iter().map(|w| w.len()).max().unwrap();

    let prefix_crc = settings.prefix.map(|p| p.1);

    for word in &words {
        // this seems slow
        let crc = xiv_crc32(word.as_bytes());
        str_to_crc.insert(word.to_string(), crc);

        let combined = if let Some(prefix_crc) = prefix_crc {
            xiv_crc32_combine(prefix_crc, crc, word.len())
        } else {
            crc
        };

        for i in 0..=max_word_len {
            let combined2 = xiv_crc32_combine(combined, 0, i);
            len_crc_map.insert((i, crc), combined2);
        }
    }

    let word_table = WordTable {
        str_to_crc,
        len_crc_map,
    };

    let words_split = words
        .chunks_exact(words.len() / settings.threads)
        .map(|c| c.to_vec())
        .collect::<Vec<_>>();

    (word_table, words_split)
}

fn test() {
    let test_full = xiv_crc32(b"g_EmissiveColor");
    assert_eq!(test_full, CRC_G_EMISSIVECOLOR);

    let test_g = xiv_crc32(b"g_");
    assert_eq!(test_g, CRC_G);

    let test_emissivecolor = xiv_crc32(b"EmissiveColor");
    assert_eq!(test_emissivecolor, CRC_EMISSIVECOLOR);

    let test_full = xiv_crc32_combine(test_g, test_emissivecolor, b"EmissiveColor".len());
    assert_eq!(test_full, CRC_G_EMISSIVECOLOR);
}

#[derive(Parser, Clone)]
struct Args {
    #[clap(short, long)]
    word_list: PathBuf,

    #[clap(short, long)]
    hash_list: PathBuf,

    #[clap(short, long, default_value = "1")]
    threads: usize,

    #[clap(long, default_value = "true")]
    print_progress: bool,

    #[clap(long, default_value = "true")]
    print_when_found: bool,

    #[clap(long, default_value = "10000000")]
    print_every: usize,

    #[clap(short, long)]
    prefix: Option<String>,
}

// borrow of partially moved deez. this sucks
#[derive(Clone)]
struct Settings {
    threads: usize,
    print_progress: bool,
    print_when_found: bool,
    print_every: usize,
    prefix: Option<(String, u32)>,
}

fn main() {
    let args = Args::parse();
    test();

    let settings = Settings {
        threads: args.threads,
        print_progress: args.print_progress,
        print_when_found: args.print_when_found,
        print_every: args.print_every,
        prefix: args.prefix.map(|p| (p.clone(), xiv_crc32(p.as_bytes()))),
    };

    let wordlist = std::fs::read_to_string(args.word_list).unwrap();
    let hashes = std::fs::read_to_string(args.hash_list).unwrap();

    let mut words: Vec<String> = wordlist.split_whitespace().map(|s| s.to_string()).collect();

    words.sort();
    words.dedup();

    println!("[dict] hashing dictionary");
    let (word_table, words) = gen_words(words, settings.clone());
    println!("[dict] starting bruteforce");

    for hash in hashes.split_whitespace() {
        let mut hash = hash;
        if hash.starts_with("0x") {
            hash = &hash[2..];
        }

        let hash = u32::from_str_radix(hash, 16).unwrap();

        let result = bruteforce_threaded(hash, &word_table, &words, settings.clone());
        if !result.is_empty() {
            let items = result.join(", ");
            println!("[result] {:x} = {}", hash, items);
        } else {
            println!("[result] {:x} = unknown", hash);
        }
    }
}
