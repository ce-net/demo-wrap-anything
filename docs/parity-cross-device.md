| tool | case | exit local / wrapped | stdout sha256 local / wrapped | stderr sha256 local / wrapped | output local / wrapped | match |
|---|---|---|---|---|---|---|
| convert | resize 50% | 0 / 0 | e3b0c44298fc / e3b0c44298fc | e3b0c44298fc / e3b0c44298fc | 11d3a4c84afe / b3d8910e9cbe | NO (different CPU: 2 pixels differ by 1/65535, see note) |
| convert | grayscale to JPEG q80 | 0 / 0 | e3b0c44298fc / e3b0c44298fc | e3b0c44298fc / e3b0c44298fc | 5201f31306ca / 5201f31306ca | yes |
| convert | rotate 90, WebP | 0 / 0 | e3b0c44298fc / e3b0c44298fc | e3b0c44298fc / e3b0c44298fc | bbbe2aaba651 / bbbe2aaba651 | yes |
| convert | truncated PNG (error path) | 1 / 1 | e3b0c44298fc / e3b0c44298fc | 6d1007d20301 / 6d1007d20301 |  /  | yes |
| pandoc | Markdown to HTML (stdout) | 0 / 0 | bce1855d5722 / bce1855d5722 | e3b0c44298fc / e3b0c44298fc |  /  | yes |
| pandoc | Markdown to reStructuredText | 0 / 0 | e3b0c44298fc / e3b0c44298fc | e3b0c44298fc / e3b0c44298fc | ee60464fa7a5 / ee60464fa7a5 | yes |
