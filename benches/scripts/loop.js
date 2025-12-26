// Loop benchmark - tests basic loop performance
// Uses modular arithmetic to prevent overflow
var sum = 0;
var mod = 1000000;
for (var i = 0; i < 1000000; i = i + 1) {
    sum = (sum + i) % mod;
}
print("sum = " + sum);
