// Fibonacci benchmark - tests function calls and recursion
function fib(n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

var result = fib(30);
print("fib(30) = " + result);
