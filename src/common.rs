use clap::Parser;
use std::{collections::HashMap, path::PathBuf};

use crate::xivcrc32::XivCrc32;

#[derive(Parser)]
pub struct Args {
    #[clap(short = 'W', long)]
    word_list: PathBuf,

    #[clap(short = 'H', long)]
    hash_list: PathBuf,

    #[clap(short = 't', long, default_value = "1")]
    threads: usize,

    #[clap(short = 'w', long, default_value = "2")]
    pub words: usize,

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

    #[clap(long, default_value = "false")]
    pub xor: bool,
}

impl Args {
    pub fn read_hashes(&self) -> Vec<u32> {
        read_hashes(&self.hash_list)
    }
}

// borrow of partially moved deez. this sucks
pub struct Settings {
    pub threads: usize,
    pub print_when_found: bool,
    pub prefix: String,
    pub prefix_hash: XivCrc32,
    pub separator: String,
    pub separator_hash: XivCrc32,
    pub suffix: String,
    pub suffix_hash: XivCrc32,
    pub max_words: usize,
    pub words: Vec<String>,
}

impl Settings {
    pub fn chunked_words(&self) -> impl Iterator<Item = &[String]> {
        self.words
            .chunks((self.words.len() + self.threads - 1) / self.threads)
    }
}

impl From<&Args> for Settings {
    fn from(args: &Args) -> Self {
        let (prefix, prefix_hash) = process_prefix(&args.prefix, &args.prefix_hash);
        let (separator, separator_hash) = process_string(&args.separator);
        let (suffix, suffix_hash) = process_string(&args.suffix);
        Settings {
            threads: args.threads,
            print_when_found: args.print_when_found,
            prefix,
            prefix_hash,
            separator,
            separator_hash,
            suffix,
            suffix_hash,
            max_words: args.words,
            words: read_words(&args.word_list),
        }
    }
}

fn read_words(path: &PathBuf) -> Vec<String> {
    let mut words = std::fs::read_to_string(path)
        .unwrap()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    words.sort();
    words.dedup();

    words
}

fn read_hashes(path: &PathBuf) -> Vec<u32> {
    std::fs::read_to_string(path)
        .unwrap()
        .split_whitespace()
        .map(|mut hash| {
            if hash.starts_with("0x") {
                hash = &hash[2..];
            }

            u32::from_str_radix(hash, 16).unwrap()
        })
        .collect::<Vec<_>>()
}

fn process_prefix(prefix: &Option<String>, prefix_hash: &Option<String>) -> (String, XivCrc32) {
    match &prefix {
        Some(s) => match &prefix_hash {
            Some(hs) => (
                format!("…{}", s),
                XivCrc32::new(u32::from_str_radix(hs, 16).unwrap(), 0) + XivCrc32::from(&s[..]),
            ),
            None => (s.clone(), XivCrc32::from(&s[..])),
        },
        None => match &prefix_hash {
            Some(hs) => (
                "…".to_string(),
                XivCrc32::new(u32::from_str_radix(hs, 16).unwrap(), 0),
            ),
            None => (String::new(), XivCrc32::default()),
        },
    }
}

fn process_string(string: &Option<String>) -> (String, XivCrc32) {
    match &string {
        Some(s) => (s.clone(), XivCrc32::from(&s[..])),
        None => (String::new(), XivCrc32::default()),
    }
}

pub fn build_lookup_tables(
    words: &Vec<String>,
    crc_to_str_suffix_hash: XivCrc32,
) -> (Vec<HashMap<String, u32>>, Vec<HashMap<u32, String>>) {
    let all_str_to_crc: HashMap<String, XivCrc32> = words
        .iter()
        .map(|s| (s.clone(), XivCrc32::from(&s[..])))
        .collect();

    let mut str_to_crc_builder: Vec<HashMap<String, u32>> = Vec::new();
    let mut crc_to_str_builder: Vec<HashMap<u32, String>> = Vec::new();
    for (word, hash) in &all_str_to_crc {
        while str_to_crc_builder.len() < word.len() {
            str_to_crc_builder.push(HashMap::new());
        }
        while crc_to_str_builder.len() < word.len() + crc_to_str_suffix_hash.len {
            crc_to_str_builder.push(HashMap::new());
        }
        str_to_crc_builder[word.len() - 1].insert(word.to_owned(), hash.crc);
        if let Some(other_word) = crc_to_str_builder[word.len() + crc_to_str_suffix_hash.len - 1]
            .insert((*hash + crc_to_str_suffix_hash).crc, word.to_owned())
        {
            println!("Collision between words {} and {}", other_word, word);
        }
    }
    (str_to_crc_builder, crc_to_str_builder)
}
