| tool | case | exit local / wrapped | stdout sha256 local / wrapped | stderr sha256 local / wrapped | output local / wrapped | match |
|---|---|---|---|---|---|---|
| convert | resize 50% | 0 / 0 | e3b0c44298fc / e3b0c44298fc | e3b0c44298fc / e3b0c44298fc | 11d3a4c84afe / 11d3a4c84afe | yes |
| convert | grayscale to JPEG q80 | 0 / 0 | e3b0c44298fc / e3b0c44298fc | e3b0c44298fc / e3b0c44298fc | 5201f31306ca / 5201f31306ca | yes |
| convert | rotate 90, WebP | 0 / 0 | e3b0c44298fc / e3b0c44298fc | e3b0c44298fc / e3b0c44298fc | bbbe2aaba651 / bbbe2aaba651 | yes |
| convert | truncated PNG (error path) | 1 / 1 | e3b0c44298fc / e3b0c44298fc | 6d1007d20301 / 6d1007d20301 |  /  | yes |
| pandoc | Markdown to HTML (stdout) | 0 / 0 | bce1855d5722 / bce1855d5722 | e3b0c44298fc / e3b0c44298fc |  /  | yes |
| pandoc | Markdown to reStructuredText | 0 / 0 | e3b0c44298fc / e3b0c44298fc | e3b0c44298fc / e3b0c44298fc | ee60464fa7a5 / ee60464fa7a5 | yes |
| jq | filter | 0 / 0 | 46fc8ef713fb / 46fc8ef713fb | e3b0c44298fc / e3b0c44298fc |  /  | yes |
| jq | invalid JSON (error path) | 4 / 4 | e3b0c44298fc / e3b0c44298fc | 648371d03d09 / 648371d03d09 |  /  | yes |
