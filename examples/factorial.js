// Factorial - demonstrates recursion and large numbers

function factorial(n) {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}

print("Factorials:");
for (var i = 0; i <= 12; i = i + 1) {
    print("  " + i + "! = " + factorial(i));
}
