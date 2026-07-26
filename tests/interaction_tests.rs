#![cfg(test)]

use karbox_downloader::interaction::{Prompt, DownloadResults};

#[test]
fn test_prompt_insert() {
    let mut prompt = Prompt::default();
    prompt.insert('a');
    assert_eq!(prompt.buffer, vec!['a']);
    assert_eq!(prompt.cursor, 1);
}

#[test]
fn test_prompt_insert_str() {
    let mut prompt = Prompt::default();
    prompt.insert_str("hello");
    assert_eq!(prompt.buffer, vec!['h', 'e', 'l', 'l', 'o']);
    assert_eq!(prompt.cursor, 5);
}

#[test]
fn test_prompt_backspace() {
    let mut prompt = Prompt::default();
    prompt.insert_str("hello");
    prompt.backspace();
    assert_eq!(prompt.buffer, vec!['h', 'e', 'l', 'l']);
    assert_eq!(prompt.cursor, 4);
}

#[test]
fn test_prompt_clear() {
    let mut prompt = Prompt::default();
    prompt.insert_str("hello");
    prompt.clear();
    assert_eq!(prompt.buffer, vec![] as Vec<char>);
    assert_eq!(prompt.cursor, 0);
}

#[test]
fn test_download_results_push() {
    let mut results = DownloadResults::default();
    results.push("Test result".to_string());
    assert_eq!(results.dr.len(), 1);
}
