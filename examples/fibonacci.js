// Fibonacci sequence - demonstrates recursion and functions

function fibonacci(n) {
    if (n <= 1) return n;
    return fibonacci(n - 1) + fibonacci(n - 2);
}

print("Fibonacci sequence:");
for (var i = 0; i <= 15; i = i + 1) {
    print("  fib(" + i + ") = " + fibonacci(i));
}
