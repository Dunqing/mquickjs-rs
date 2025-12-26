// Strings - demonstrates string methods

var str = "  Hello, World!  ";

print("Original: '" + str + "'");
print("Trimmed: '" + str.trim() + "'");
print("Upper: " + str.toUpperCase());
print("Lower: " + str.toLowerCase());
print("Length: " + str.length);

var message = "The quick brown fox jumps over the lazy dog";

print("\nSearching in: '" + message + "'");
print("indexOf('fox'): " + message.indexOf("fox"));
print("includes('quick'): " + message.includes("quick"));
print("startsWith('The'): " + message.startsWith("The"));
print("endsWith('dog'): " + message.endsWith("dog"));

// Slice and substring
print("\nSlicing:");
print("slice(4, 9): '" + message.slice(4, 9) + "'");
print("slice(-3): '" + message.slice(-3) + "'");

// Split
var words = message.split(" ");
print("\nWords: " + words.length);
print("First 3: " + words.slice(0, 3).join(", "));

// Replace
print("\nReplace 'fox' with 'cat': " + message.replace("fox", "cat"));

// Padding
var num = "42";
print("\nPadding:");
print("padStart(5, '0'): " + num.padStart(5, "0"));
print("padEnd(5, '!'): " + num.padEnd(5, "!"));
