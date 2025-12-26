// Sieve of Eratosthenes - classic benchmark
function sieve(n) {
    var primes = [];
    for (var i = 0; i <= n; i = i + 1) {
        primes.push(true);
    }
    primes[0] = false;
    primes[1] = false;

    for (var i = 2; i * i <= n; i = i + 1) {
        if (primes[i]) {
            for (var j = i * i; j <= n; j = j + i) {
                primes[j] = false;
            }
        }
    }

    var count = 0;
    for (var i = 0; i <= n; i = i + 1) {
        if (primes[i]) count = count + 1;
    }
    return count;
}

var result = sieve(100000);
print("primes up to 100000: " + result);
