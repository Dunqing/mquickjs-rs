// Closure benchmark - tests closure creation and invocation
function makeAdder(x) {
    function adder(y) {
        return x + y;
    }
    return adder;
}

var adders = [];
for (var i = 0; i < 1000; i = i + 1) {
    adders.push(makeAdder(i));
}

var sum = 0;
for (var i = 0; i < adders.length; i = i + 1) {
    sum = sum + adders[i](i);
}
print("closure sum = " + sum);
