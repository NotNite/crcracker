use std::collections::HashMap;

use crate::common::{build_lookup_tables, Settings};
use crate::xivcrc32::XivCrc32;

fn bruteforce(
    target_hash: u32,
    all_bare_str_to_crc: &Vec<HashMap<String, u32>>,
    all_suffixed_crc_to_str: &Vec<HashMap<u32, String>>,
    prefixed_str_to_crc: HashMap<String, XivCrc32>,
    settings: &Settings,
    tx: std::sync::mpsc::Sender<String>,
) {
    for (word_one, hash_one) in &prefixed_str_to_crc {
        if (*hash_one + settings.suffix_hash).crc == target_hash {
            let msg = format!("{}{}{}", settings.prefix, word_one, settings.suffix);

            if settings.print_when_found {
                println!("[match] {:x} = {}", target_hash, msg);
            }

            tx.send(msg).unwrap();
        }

        if settings.max_words == 2 {
            let mut blank_hash = *hash_one + settings.separator_hash;
            for suffixed_crc_to_str in all_suffixed_crc_to_str {
                blank_hash += XivCrc32::zero(1);
                let hash_two = blank_hash.crc ^ target_hash;
                if let Some(word_two) = suffixed_crc_to_str.get(&hash_two) {
                    let msg = format!(
                        "{}{}{}{}{}",
                        settings.prefix, word_one, settings.separator, word_two, settings.suffix
                    );

                    if settings.print_when_found {
                        println!("[match] {:x} = {}", target_hash, msg);
                    }

                    tx.send(msg).unwrap();
                }
            }
        } else if settings.max_words >= 3 {
            let mut blank_hash = *hash_one;
            for bare_str_to_crc in all_bare_str_to_crc {
                blank_hash += XivCrc32::zero(1);
                for (word_two, hash_two) in bare_str_to_crc {
                    let combined_hash = blank_hash ^ XivCrc32::new(*hash_two, 0);
                    if (combined_hash + settings.suffix_hash).crc == target_hash {
                        let msg = format!(
                            "{}{}{}{}{}",
                            settings.prefix,
                            word_one,
                            settings.separator,
                            word_two,
                            settings.suffix
                        );

                        if settings.print_when_found {
                            println!("[match] {:x} = {}", target_hash, msg);
                        }

                        tx.send(msg).unwrap();
                    }

                    let mut blank_hash_two = combined_hash + settings.separator_hash;
                    for suffixed_crc_to_str in all_suffixed_crc_to_str {
                        blank_hash_two += XivCrc32::zero(1);
                        let hash_three = blank_hash_two.crc ^ target_hash;
                        if let Some(word_three) = suffixed_crc_to_str.get(&hash_three) {
                            let msg = format!(
                                "{}{}{}{}{}{}{}",
                                settings.prefix,
                                word_one,
                                settings.separator,
                                word_two,
                                settings.separator,
                                word_three,
                                settings.suffix
                            );

                            if settings.print_when_found {
                                println!("[match] {:x} = {}", target_hash, msg);
                            }

                            tx.send(msg).unwrap();
                        }
                    }
                }
            }
        }
    }
}

pub fn bruteforce_threaded(target_hash: u32, settings: &Settings) -> Vec<String> {
    let (bare_str_to_crc, suffixed_crc_to_str) =
        build_lookup_tables(&settings.words, settings.suffix_hash);

    std::thread::scope(|s| {
        let (tx, rx) = std::sync::mpsc::channel();

        settings.chunked_words().for_each(|our_words| {
            let our_prefixed_str_to_crc: HashMap<String, XivCrc32> = our_words
                .iter()
                .map(|s| (s.clone(), settings.prefix_hash + XivCrc32::from(&s[..])))
                .collect();

            let worker_tx = tx.clone();

            s.spawn(|| {
                bruteforce(
                    target_hash,
                    &bare_str_to_crc,
                    &suffixed_crc_to_str,
                    our_prefixed_str_to_crc,
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
