// Loop benchmark - tests basic loop performance
var sum = 0;
for (var i = 0; i < 1000000; i = i + 1) {
    sum = sum + i;
}
print("sum = " + sum);
