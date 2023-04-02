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
    print_when_found: bool,
) {
    let mut current_cycle = 0;
    let mut last_cycle = 0;

    for word_one in our_words {
        for word_two in full_words {
            current_cycle += 1;
            if current_cycle % 10000000 == 0 {
                let diff = current_cycle - last_cycle;
                tx.send(Message::Progress(diff)).ok();
                last_cycle = current_cycle;
            }

            // I'm sleep deprived so I probably misunderstood the speedup here
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
    words: &Vec<Vec<String>>,
    print_progress: bool,
    print_when_found: bool,
) -> Vec<String> {
    let mut handles = vec![];
    let mut mpsc_rx = vec![];
    let num_threads = words.len();

    let words_flat = words.iter().flatten().cloned().collect::<Vec<_>>();
    let mut cycles = 0;

    (0..num_threads).for_each(|i| {
        // .clone() makes me sad but i am tired
        let our_words = words[i].clone();
        let words_flat = words_flat.clone();
        let word_table = word_table.clone();

        let (tx, rx) = std::sync::mpsc::channel();

        cycles += our_words.len() * words_flat.len();

        let handle = std::thread::spawn(move || {
            bruteforce(
                target_hash,
                &word_table,
                &words_flat,
                &our_words,
                tx,
                print_when_found,
            );
        });

        handles.push(handle);
        mpsc_rx.push(rx);
    });

    let mut ret_none = 0;
    let mut progress: usize = 0;
    let mut last_progress = 0;

    if print_progress {
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
                        if diff >= 1000000 {
                            let clean_progress = progress - (progress % 1000000);
                            let percent = (clean_progress as f64 / cycles as f64) * 100.0;
                            if print_progress {
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

        if ret_none == num_threads {
            return possible;
        }
    }
}

fn gen_words(words: Vec<String>, num_threads: usize) -> (WordTable, Vec<Vec<String>>) {
    let mut str_to_crc = HashMap::new();
    let mut len_crc_map = HashMap::new();

    let max_word_len = words.iter().map(|w| w.len()).max().unwrap();

    for word in &words {
        // this seems slow
        let crc = xiv_crc32(word.as_bytes());
        str_to_crc.insert(word.to_string(), crc);

        let combined = xiv_crc32_combine(CRC_G, crc, word.len());
        for i in 0..(max_word_len + 1) {
            let combined2 = xiv_crc32_combine(combined, 0, i);
            len_crc_map.insert((i, crc), combined2);
        }
    }

    let word_table = WordTable {
        str_to_crc,
        len_crc_map,
    };

    let words_split = words
        .chunks(words.len() / num_threads)
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

#[derive(Parser)]
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
}

fn main() {
    test();

    let args = Args::parse();

    let wordlist = std::fs::read_to_string(args.word_list).unwrap();
    let hashes = std::fs::read_to_string(args.hash_list).unwrap();
    let num_threads = args.threads;

    let mut words: Vec<String> = wordlist.split_whitespace().map(|s| s.to_string()).collect();

    words.sort();
    words.dedup();

    println!("[dict] hashing dictionary");
    let (word_table, words) = gen_words(words, num_threads);
    println!("[dict] starting bruteforce");

    for hash in hashes.split_whitespace() {
        let mut hash = hash;
        if hash.starts_with("0x") {
            hash = &hash[2..];
        }

        let hash = u32::from_str_radix(hash, 16).unwrap();

        let result = bruteforce_threaded(
            hash,
            &word_table,
            &words,
            args.print_progress,
            args.print_when_found,
        );
        if !result.is_empty() {
            let items = result.join(", ");
            println!("[result] {:x} = {}", hash, items);
        } else {
            println!("[result] {:x} = unknown", hash);
        }
    }
}
