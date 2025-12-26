// Regular Expressions - demonstrates RegExp

var text = "The quick brown fox jumps over the lazy dog";

// Test for match
var foxRegex = new RegExp("fox");
print("Contains 'fox': " + foxRegex.test(text));

var catRegex = new RegExp("cat");
print("Contains 'cat': " + catRegex.test(text));

// Case insensitive
var theRegex = new RegExp("THE", "i");
print("Contains 'THE' (case insensitive): " + theRegex.test(text));

// Exec - get match details
var wordRegex = new RegExp("quick");
var match = wordRegex.exec(text);
if (match) {
    print("\nFound 'quick': '" + match[0] + "'");
}

// String.search
print("\nSearch results:");
print("  'brown' at index: " + text.search(new RegExp("brown")));
print("  'xyz' at index: " + text.search(new RegExp("xyz")));

// Simple patterns
print("\nPattern matching:");
var hasDigits = new RegExp("[0-9]");
print("  'abc123' has digits: " + hasDigits.test("abc123"));
print("  'abcdef' has digits: " + hasDigits.test("abcdef"));
