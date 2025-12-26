// Exceptions - demonstrates try/catch/finally

function divide(a, b) {
    if (b === 0) {
        throw new Error("Division by zero");
    }
    return a / b;
}

// Basic try/catch
try {
    print("10 / 2 = " + divide(10, 2));
    print("10 / 0 = " + divide(10, 0));
} catch (e) {
    print("Caught error: " + e.message);
}

// Try/catch/finally
print("\nWith finally:");
try {
    print("Trying something risky...");
    throw new Error("Oops!");
} catch (e) {
    print("Caught: " + e.message);
} finally {
    print("Finally block always runs");
}

// Nested try/catch
print("\nNested exceptions:");
try {
    try {
        throw new TypeError("Type mismatch");
    } catch (e) {
        print("Inner catch: " + e.name);
        throw new RangeError("Out of range");
    }
} catch (e) {
    print("Outer catch: " + e.name + " - " + e.message);
}

// Different error types
print("\nError types:");
var errors = [
    new Error("generic"),
    new TypeError("type"),
    new RangeError("range"),
    new ReferenceError("reference"),
    new SyntaxError("syntax")
];

for (var i = 0; i < errors.length; i = i + 1) {
    print("  " + errors[i].name + ": " + errors[i].message);
}
