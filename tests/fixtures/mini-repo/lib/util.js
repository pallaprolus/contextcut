// Full-line comment, removable.
const snippet = `
```python
fenced code inside a string
```
`;
const url = "https://example.com"; // trailing comment stays in v0.1

function dump() {
  console.log(snippet, url);
}

module.exports = { dump };
