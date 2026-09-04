use std::collections::HashMap;

use crate::common::{build_lookup_tables, Settings};
use crate::xivcrc32::XivCrc32;

fn bruteforce(
    target_hash: u32,
    all_crc_to_str: &Vec<HashMap<u32, String>>,
    str_to_crc: HashMap<String, XivCrc32>,
    settings: &Settings,
    tx: std::sync::mpsc::Sender<(String, String)>,
) {
    println!("hello {:x}", target_hash);
    for (word, hash) in &str_to_crc {
        let searched_hash = hash.crc ^ target_hash;
        for crc_to_str in all_crc_to_str {
            if let Some(other_word) = crc_to_str.get(&searched_hash) {
                if other_word > word {
                    if settings.print_when_found {
                        println!(
                            "{}{} {}{}",
                            word, settings.suffix, other_word, settings.suffix
                        );
                    }

                    tx.send((
                        format!("{}{}", word, settings.suffix),
                        format!("{}{}", other_word, settings.suffix),
                    ))
                    .unwrap();
                }
            }
        }
    }
}

pub fn bruteforce_threaded(target_hash: u32, settings: &Settings) -> Vec<(String, String)> {
    let (_, crc_to_str) = build_lookup_tables(&settings.words, settings.suffix_hash);

    std::thread::scope(|s| {
        let (tx, rx) = std::sync::mpsc::channel();

        settings.chunked_words().for_each(|our_words| {
            let our_str_to_crc: HashMap<String, XivCrc32> = our_words
                .iter()
                .map(|s| (s.clone(), XivCrc32::from(&s[..]) + settings.suffix_hash))
                .collect();

            let worker_tx = tx.clone();

            s.spawn(|| {
                bruteforce(
                    target_hash,
                    &crc_to_str,
                    our_str_to_crc,
                    settings,
                    worker_tx,
                );
            });
        });

        drop(tx);

        let mut possible = vec![];
        while let Ok(result) = rx.recv() {
            possible.push(result);
        }
        possible.sort();

        possible
    })
}
