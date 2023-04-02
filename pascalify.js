// helper script to process words into pascal case
// node pascalify.js input.txt output.txt
const fs = require("fs");

const input = process.argv[2];
const output = process.argv[3];

const words = fs.readFileSync(input, "utf8")
  .trim()
  .split("\n")
  // CRLF sucks
  .map(word => word.trim());

for (const word of words) {
  const pascal = word[0].toUpperCase() + word.slice(1).toLowerCase();
  fs.appendFileSync(output, pascal + "\n");
}
