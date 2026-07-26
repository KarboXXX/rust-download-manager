#![cfg(test)]

use karbox_downloader::downloading::parse_filename_from_url;

#[test]
fn test_parse_filename_from_url() {
    let url = "https://example.com/2131345254234443.txt";
    assert_eq!(parse_filename_from_url(url.to_string()), "2131345254234443.txt");

    let url_with_trailing_slash = "https://example.com/____________.txt/";
    assert_eq!(parse_filename_from_url(url_with_trailing_slash.to_string()), "____________.txt");

    let url_with_path = "https://example.com/path/to/2391840--_938asknfaojnsdo.gjidao.656.txt.txt";
    assert_eq!(parse_filename_from_url(url_with_path.to_string()), "2391840--_938asknfaojnsdo.gjidao.656.txt.txt");
}
