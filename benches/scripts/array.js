// Array benchmark - tests array operations
var arr = [];
for (var i = 0; i < 10000; i = i + 1) {
    arr.push(i);
}

var sum = 0;
for (var i = 0; i < arr.length; i = i + 1) {
    sum = sum + arr[i];
}
print("array sum = " + sum);
